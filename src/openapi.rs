use aide::generate::GenContext;
use aide::openapi::{
    HeaderStyle, MediaType, Parameter, ParameterData, ParameterSchemaOrContent, PathStyle,
    RequestBody, SchemaObject,
};
use aide::operation::{OperationInput, add_parameters, set_body};
use indexmap::IndexMap;
use schemars::Schema;
use schemars::json_schema;
pub struct ApiKeyHeader;

impl OperationInput for ApiKeyHeader {
    fn operation_input(ctx: &mut GenContext, operation: &mut aide::openapi::Operation) {
        let schema = ctx.schema.subschema_for::<String>();
        add_parameters(
            ctx,
            operation,
            [Parameter::Header {
                parameter_data: ParameterData {
                    name: "X-API-Key".to_string(),
                    description: Some("API key for preload endpoint.".to_string()),
                    required: true,
                    format: ParameterSchemaOrContent::Schema(SchemaObject {
                        json_schema: schema,
                        example: None,
                        external_docs: None,
                    }),
                    extensions: Default::default(),
                    deprecated: None,
                    example: None,
                    examples: IndexMap::default(),
                    explode: None,
                },
                style: HeaderStyle::Simple,
            }],
        );
    }
}

pub struct BinaryBody;

impl OperationInput for BinaryBody {
    fn operation_input(ctx: &mut GenContext, operation: &mut aide::openapi::Operation) {
        let schema: Schema = json_schema!({
            "type": "string",
            "format": "binary"
        });
        set_body(
            ctx,
            operation,
            RequestBody {
                description: Some("Binary image payload.".to_string()),
                content: IndexMap::from_iter([(
                    "application/octet-stream".to_string(),
                    MediaType {
                        schema: Some(SchemaObject {
                            json_schema: schema,
                            example: None,
                            external_docs: None,
                        }),
                        ..Default::default()
                    },
                )]),
                required: true,
                extensions: Default::default(),
            },
        );
    }
}

pub struct ImageIdParam;

impl OperationInput for ImageIdParam {
    fn operation_input(ctx: &mut GenContext, operation: &mut aide::openapi::Operation) {
        let schema = ctx.schema.subschema_for::<String>();
        add_parameters(
            ctx,
            operation,
            [Parameter::Path {
                parameter_data: ParameterData {
                    name: "id".to_string(),
                    description: Some("Image identifier.".to_string()),
                    required: true,
                    format: ParameterSchemaOrContent::Schema(SchemaObject {
                        json_schema: schema,
                        example: None,
                        external_docs: None,
                    }),
                    extensions: Default::default(),
                    deprecated: None,
                    example: None,
                    examples: IndexMap::default(),
                    explode: None,
                },
                style: PathStyle::Simple,
            }],
        );
    }
}
