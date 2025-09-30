use std::process::exit;

use edgesearch_client::http::Client;
use edgesearch_client::query::{QueryBuilder, QueryExpr};
use edgesearch_client::Result;

fn main() -> Result<()> {
    // Expect the base URL to be passed as the first argument
    // and the API key after
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <base_url> <api_key>", args[0]);
        exit(1);
    }

    let _url = args[1].clone();
    let base_url = url::Url::parse(&_url)
        .map_err(|e| {
            eprintln!("Invalid base URL: {}", e);
            exit(1);
        })
        .unwrap();

    // Create a client with reqwest HTTP implementation
    let client = Client::new(base_url.origin().ascii_serialization().to_string())
        .with_api_key(args[2].to_string());

    // Check status
    let status = client.status()?;
    println!("API Status: ready = {}", status.ready);

    // Create an index
    let index = client.create_index("my-index")?;
    println!(
        "Created index: {} with {} docs",
        index.index, index.docs_count
    );

    // Add some documents
    let doc1 = client.add_document(
        "my-index",
        "Hello world content about programming".to_string(),
        None,
        None,
    )?;
    println!("Added document 1: {}", doc1.uuid);

    let doc2 = client.add_document(
        "my-index",
        "World peace and harmony".to_string(),
        None,
        None,
    )?;

    println!("Added document 2: {}", doc2.uuid);

    let doc3 = client.add_document(
        "my-index",
        "Programming tutorials and guides".to_string(),
        None,
        None,
    )?;
    println!("Added document 3: {}", doc3.uuid);

    // Basic search for documents
    let results = client.search("my-index", "\"programming\"", Some(true))?;
    println!("\nBasic search found {} documents", results.document_count);

    for result in &results.matches {
        println!("- Document {}: score={:.2}", result.doc_id, result.score);
        if let Some(body) = &result.body {
            println!("  Body: {}", body);
        }
    }

    // Using QueryExpr directly
    let query_expr = QueryExpr::word("programming")
        .or(QueryExpr::word("world"))
        .and(QueryExpr::word("hello").not());

    println!("\nQuery expression: {}", query_expr);
    let expr_results = client.search_expr("my-index", &query_expr, Some(true))?;
    println!(
        "Expression search found {} documents",
        expr_results.document_count
    );

    // Using QueryBuilder fluently
    let builder = QueryBuilder::word("programming")
        .or("world")
        .and_expr(QueryExpr::word("hello").not());

    if let Some(query) = builder.to_query_string() {
        println!("\nBuilt query: {}", query);
        let builder_results = client.search("my-index", &query, Some(true))?;
        println!(
            "Builder search found {} documents",
            builder_results.document_count
        );
    }

    // Complex query example
    let complex_query = QueryBuilder::word("programming")
        .and("tutorials")
        .or_expr(QueryExpr::word("world").and(QueryExpr::word("peace")));

    if let Some(built_query) = complex_query.to_query_string() {
        println!("\nComplex query: {}", built_query);
        let complex_results = client.search("my-index", &built_query, Some(false))?;
        println!(
            "Complex search found {} documents",
            complex_results.document_count
        );
    }

    // Update the document
    let update_response =
        client.update_document("my-index", &doc1.uuid, "Updated content".to_string())?;
    println!("\nDocument updated: revision={}", update_response.revision);

    // Get a specific document
    let retrieved_doc = client.get_document("my-index", &doc1.uuid)?;
    println!("Retrieved document body: {:?}", retrieved_doc.document_body);

    // Search for a keyword
    let keyword_response = client.get_keyword("my-index", "programming")?;
    println!(
        "Keyword '{}' found in {} documents",
        keyword_response.keyword, keyword_response.document_count
    );

    // List all indexes
    let indexes = client.list_indexes()?;
    println!("Available indexes: {:?}", indexes);

    // Get index info
    let index_info = client.get_index("my-index")?;
    println!(
        "Index info: {} docs, version {}",
        index_info.docs_count, index_info.version
    );

    // Delete the documents
    client.delete_document("my-index", &doc1.uuid)?;
    client.delete_document("my-index", &doc2.uuid)?;
    client.delete_document("my-index", &doc3.uuid)?;
    println!("Documents deleted");

    // Delete the index
    let deleted = client.delete_index("my-index")?;
    println!("Index deleted: {}", deleted.deleted);

    Ok(())
}
