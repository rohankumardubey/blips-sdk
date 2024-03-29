mod introspection_schema;

use std::fs::File;
use std::io::{BufReader, Write};
use std::process::Command;

use heck::{ToPascalCase, ToSnakeCase};

use introspection_schema::{
    Field, GraphQlFullType, GraphQlTypeRef, IntrospectionResponse, IntrospectionSchema,
};

fn resolve_type_name(ty: &GraphQlTypeRef) -> &String {
    match ty {
        GraphQlTypeRef::Scalar { name }
        | GraphQlTypeRef::Object { name }
        | GraphQlTypeRef::Interface { name }
        | GraphQlTypeRef::Union { name }
        | GraphQlTypeRef::Enum { name }
        | GraphQlTypeRef::InputObject { name } => name,
        GraphQlTypeRef::NonNull(boxed) | GraphQlTypeRef::List(boxed) => {
            resolve_type_name(&boxed.of_type)
        }
    }
}

fn render_type_name(ty: &GraphQlTypeRef) -> String {
    match ty {
        GraphQlTypeRef::Scalar { name }
        | GraphQlTypeRef::Object { name }
        | GraphQlTypeRef::Interface { name }
        | GraphQlTypeRef::Union { name }
        | GraphQlTypeRef::Enum { name }
        | GraphQlTypeRef::InputObject { name } => name.to_owned(),
        GraphQlTypeRef::NonNull(boxed) => format!("{}!", render_type_name(&boxed.of_type)),
        GraphQlTypeRef::List(boxed) => format!("[{}]", render_type_name(&boxed.of_type)),
    }
}

fn sanitize_name(name: String) -> String {
    name.replace("OAuth", "Oauth")
}

#[derive(Debug)]
struct QueryType {
    fields: Vec<Field>,
}

impl QueryType {
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }
}

impl TryFrom<&IntrospectionSchema> for QueryType {
    type Error = &'static str;

    fn try_from(schema: &IntrospectionSchema) -> Result<Self, Self::Error> {
        let query_name = &schema.query_type.name;

        let query_type = schema
            .types
            .iter()
            .find_map(|ty| match ty {
                GraphQlFullType::Object(object) if &object.name == query_name => Some(object),
                _ => None,
            })
            .ok_or("No Query type found")?;

        Ok(Self {
            fields: query_type.fields.to_vec(),
        })
    }
}

#[derive(Debug)]
struct MutationType {
    fields: Vec<Field>,
}

impl MutationType {
    pub fn from_schema(schema: &IntrospectionSchema) -> Result<Option<Self>, &'static str> {
        let mutation_type = match &schema.mutation_type {
            Some(mutation_type) => mutation_type,
            None => return Ok(None),
        };

        let mutation_name = &mutation_type.name;

        let mutation_type = schema
            .types
            .iter()
            .find_map(|ty| match ty {
                GraphQlFullType::Object(object) if &object.name == mutation_name => Some(object),
                _ => None,
            })
            .ok_or("No Mutation type found")?;

        Ok(Some(MutationType {
            fields: mutation_type.fields.to_vec(),
        }))
    }

    pub fn fields(&self) -> &[Field] {
        &self.fields
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum GraphQlOperation {
    Query,
    Mutation,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let schema_file = File::open("schema.json")?;
    let buf_reader = BufReader::new(schema_file);

    let schema_query: IntrospectionResponse = serde_json::from_reader(buf_reader)?;

    let schema = schema_query.data.schema;

    let query = QueryType::try_from(&schema)?;
    let mutation = MutationType::from_schema(&schema)?;

    let mut emitted_graphql_modules: Vec<String> = Vec::new();
    let mut generated_client_impls: Vec<String> = Vec::new();

    let mut fields = Vec::new();
    fields.extend(
        query
            .fields()
            .iter()
            .map(|field| (GraphQlOperation::Query, field)),
    );

    if let Some(mutation) = &mutation {
        fields.extend(
            mutation
                .fields()
                .iter()
                .map(|field| (GraphQlOperation::Mutation, field)),
        );
    }

    for (operation, field) in fields {
        let field_type_name = resolve_type_name(&field.ty);

        let has_args = !field.args.is_empty();
        let args_list = field
            .args
            .iter()
            .map(|arg| {
                format!(
                    "${}: {}",
                    arg.name.to_snake_case(),
                    render_type_name(&arg.ty)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let applied_args_list = field
            .args
            .iter()
            .map(|arg| format!("{}: ${}", arg.name, arg.name.to_snake_case()))
            .collect::<Vec<_>>()
            .join(", ");

        let field_type = schema
            .types
            .iter()
            .find(|ty| ty.name().as_ref() == Some(&field_type_name))
            .expect(&format!("No type found for field '{}'", field_type_name));

        let mut fragment_field_names = Vec::new();
        if let GraphQlFullType::Object(object) = &field_type {
            for sub_field in &object.fields {
                let sub_field_type_name = resolve_type_name(&sub_field.ty);

                let sub_field_type = schema
                    .types
                    .iter()
                    .find(|ty| ty.name().as_ref() == Some(&sub_field_type_name))
                    .expect(&format!(
                        "No type found for sub field '{}'",
                        sub_field_type_name
                    ));

                if let GraphQlFullType::Scalar(_) = sub_field_type {
                    fragment_field_names.push(sub_field.name.clone());
                }
            }
        }

        let contents = format!(
            r#"
{operation} {query_name}{args_list} {{
    {field_name}{applied_args_list} {{
        ...{fragment_name}
    }}
}}

fragment {fragment_name} on {fragment_name} {{
    __typename
    {fragment_fields}
}}
            "#,
            operation = match operation {
                GraphQlOperation::Query => "query",
                GraphQlOperation::Mutation => "mutation",
            },
            query_name = sanitize_name(field.name.clone()).to_pascal_case(),
            args_list = if has_args {
                format!("({})", args_list)
            } else {
                String::new()
            },
            applied_args_list = if has_args {
                format!("({})", applied_args_list)
            } else {
                String::new()
            },
            field_name = field.name,
            fragment_name = field_type_name.to_pascal_case(),
            fragment_fields = fragment_field_names.join("\n    ")
        );

        let rust_module_name = sanitize_name(field.name.clone()).to_snake_case();

        let mut graphql_file = File::create(format!(
            "crates/blips/src/graphql/generated/{}.graphql",
            rust_module_name
        ))?;

        graphql_file.write_all(contents.trim().as_bytes())?;

        emitted_graphql_modules.push(rust_module_name.clone());

        let generated_client_impl = format!(
            r#"
    pub async fn {fn_name}(
        &self,
        variables: crate::graphql::{module_name}::Variables,
    ) -> Result<crate::graphql::{module_name}::ResponseData, reqwest::Error> {{
        let response_body = self
            .post_graphql::<crate::graphql::{operation_name}>(variables)
            .await?;

        Ok(response_body.data.expect("No data"))
    }}
            "#,
            fn_name = sanitize_name(field.name.clone()).to_snake_case(),
            module_name = rust_module_name,
            operation_name = sanitize_name(field.name.clone()).to_pascal_case()
        )
        .trim()
        .to_string();

        generated_client_impls.push(generated_client_impl);
    }

    emitted_graphql_modules.sort_unstable();

    for emitted_graphql_module in &emitted_graphql_modules {
        let mut generate_command = Command::new("graphql-client");

        generate_command
            .arg("generate")
            .arg("--schema-path=schema.json")
            .arg("--custom-scalars-module=crate::graphql::custom_scalars")
            .arg("--response-derives=Debug")
            .arg(format!(
                "crates/blips/src/graphql/generated/{}.graphql",
                emitted_graphql_module
            ));

        generate_command.status()?;
    }

    let mut generated_module_file = File::create("crates/blips/src/graphql/generated.rs")?;

    generated_module_file.write_all(
        (emitted_graphql_modules
            .iter()
            .map(|module_name| format!("pub mod {};", module_name))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n")
            .as_bytes(),
    )?;

    let mut generated_graphql_module_file = File::create("crates/blips/src/graphql.rs")?;

    generated_graphql_module_file.write_all(
        format!(
            r#"
mod custom_scalars;
mod generated;

// Auto-generated:
{}
            "#,
            emitted_graphql_modules
                .iter()
                .map(|module_name| format!("pub use generated::{}::*;", module_name))
                .collect::<Vec<_>>()
                .join("\n")
        )
        .trim()
        .as_bytes(),
    )?;

    let mut generated_client_file = File::create("crates/blips/src/client_generated.rs")?;

    generated_client_file.write_all(
        format!(
            r#"
impl crate::BlipsClient {{
    {impls}
}}
            "#,
            impls = generated_client_impls.join("\n\n")
        )
        .trim()
        .as_bytes(),
    )?;

    Command::new("cargo").arg("fmt").status()?;

    Ok(())
}
