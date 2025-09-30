use lingua::IsoCode639_1;
use worker::{Request, Response, Result, RouteContext};

use crate::{data::document::Document, http::ErrorResponse, util::kv::get_kv_data_store};

pub async fn handle_get_document(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(index) = ctx.param("index") {
        if let Some(doc_id) = ctx.param("id") {
            let store = get_kv_data_store(&ctx);
            if let Ok(document) = Document::from_remote(&store, index, doc_id.to_string()).await {
                return Response::from_json(&document);
            } else {
                return Response::error(
                    ErrorResponse {
                        error: "Document not found".into(),
                    },
                    404,
                );
            }
        }
        return Response::error(
            ErrorResponse {
                error: "Missing document ID".into(),
            },
            400,
        );
    }

    return Response::error(
        ErrorResponse {
            error: "Missing index name".into(),
        },
        400,
    );
}

#[derive(serde::Deserialize)]
struct AddDocumentQueryParams {
    lang: Option<IsoCode639_1>,
    format: Option<String>,
}

#[derive(serde::Serialize)]
struct UpdateDocumentResponse {
    pub updated: bool,
    pub scores: Vec<(String, f64)>,
    pub revision: u32,
}

pub async fn handle_update_document(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(index) = ctx.param("index") {
        if let Some(doc_id) = ctx.param("id") {
            let store = get_kv_data_store(&ctx);
            let document_result = Document::from_remote(&store, index, doc_id.to_string()).await;
            if document_result.is_err() {
                return Response::error(
                    ErrorResponse {
                        error: "Document not found".into(),
                    },
                    404,
                );
            }

            let query = req.query::<AddDocumentQueryParams>()?;
            let mut document = document_result.unwrap();
            let document_body = req.text().await?;
            let env = &ctx.env;
            let revision = document
                .update(&store, env, document_body, query.format, false)
                .await
                .unwrap();

            return Response::from_json(&UpdateDocumentResponse {
                updated: true,
                scores: document.keywords.unwrap(),
                revision: revision,
            });
        }
        return Response::error(
            ErrorResponse {
                error: "Missing document ID".into(),
            },
            400,
        );
    }

    return Response::error(
        ErrorResponse {
            error: "Missing index name".into(),
        },
        400,
    );
}

pub async fn handle_add_document(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(index) = ctx.param("index") {
        let mut document: Document;
        if let Some(id) = ctx.param("id") {
            if !Document::is_valid_id(&id) {
                return Response::error(
                    ErrorResponse {
                        error: "Invalid document ID format. Must match [a-zA-Z0-9-_]+".into(),
                    },
                    400,
                );
            }
            document = Document::new_with_id(index, &id);
        } else {
            document = Document::new(index);
        }

        if let Ok(document_body) = req.text().await {
            let env = &ctx.env;
            let store = get_kv_data_store(&ctx);

            // See if the document exists already
            let existing_doc = Document::from_remote(&store, index, document.get_uuid()).await;
            if existing_doc.is_ok() {
                return Response::error(
                    ErrorResponse {
                        error: "This document already exists".into(),
                    },
                    500,
                );
            }

            let query = req.query::<AddDocumentQueryParams>()?;
            document.set_language(query.lang.unwrap_or(IsoCode639_1::EN));
            let document = document
                .update(&store, env, document_body, query.format, false)
                .await;

            if document.is_err() {
                return Response::error(
                    ErrorResponse {
                        error: format!("Failed to add document: {}", document.err().unwrap()),
                    },
                    500,
                );
            }

            return Response::from_json(&document.unwrap());
        } else {
            return Response::error(
                ErrorResponse {
                    error: "Invalid document format".into(),
                },
                400,
            );
        }
    }
    return Response::from_bytes("Not implemented".into());
}

#[derive(serde::Serialize)]
struct DeleteDocumentResponse {
    pub deleted: bool,
}

pub async fn handle_delete_document(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(index) = ctx.param("index") {
        let mut document: Document;
        if let Some(id) = ctx.param("id") {
            if !Document::is_valid_id(&id) {
                return Response::error(
                    ErrorResponse {
                        error: "Invalid document ID format. Must match [a-zA-Z0-9-_]+".into(),
                    },
                    400,
                );
            }
            document = Document::new_with_id(index, &id);
            let store = get_kv_data_store(&ctx);
            if let Ok(_) = document.delete(&store).await {
                return Response::from_json(&serde_json::json!({
                    "deleted": true,
                }));
            } else {
                return Response::error(
                    ErrorResponse {
                        error: "Failed to delete document".into(),
                    },
                    500,
                );
            }
        } else {
            return Response::error(
                ErrorResponse {
                    error: "Missing document ID".into(),
                },
                400,
            );
        }
    }

    return Response::error(
        ErrorResponse {
            error: "Missing index name".into(),
        },
        400,
    );
}
