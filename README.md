# EdgeSearch

### A document store with complex full-text keyword search for Cloudflare Workers and KV 

> Ever wanted to do a complex keyword search on a large volume of documents without spending $400/mo for Elastic, or crying over PostgreSQL "full-text"?

> Just need a document store that will process as many uploaded documents as you have available ports on your computer, that will keep them forever?

> Broke boy with no money for something fancier?

These are the challenges EdgeSearch aims to solve. Deploy it to your Cloudflare Workers account with a few commands, and start uploading whatever documents you want (24MB limit). 

When you upload a document, EdgeSearch will automtically identify keywords in your document, index and score those keywords! As soon as KV writes the value globally, the document is instantly available in any future search queries.

## Features
- [Hash table](https://en.wikipedia.org/wiki/Hash_table) keyword sharding for minimizing data loss over slow KV replication
- [Lingua](https://docs.rs/lingua/latest/lingua/index.html) automatic language detection on documents
  - EN, can be recompiled to support other languages
  - Limited to EN by default to keep under the 2MB free limit
- Runs entirely on Cloudflare Workers + KV
  - Cloudflare-scale reading AND writing
- Custom document identifiers
- Includes a Durable Object for processing extremely large searches
- Easily segment documents and keywords into named indexes
- Simple expressive query language: `~("pop" && "crave") || "tiktok"`
- Near-instant single keyword queries `/:index/keyword/:keyword`
- Near-instant document queries: `/:index/doc/:id`
- Supports files up of to ~24MB

## Bonus Features
- Will definitely burn through your entire Cloudflare KV free plan in a day
- There is technically no limit to how many 25MB documents KV will store (and how much you can pay them)
- Searching even complex queries happens very quickly
- Expensive queries do not impact the performance of other queries

## KV Usage

The number of KV read/write commands that will be executed depends on a number of factors, most significantly the `N_SHARDS` configuration option.

We think we've chosen a reasonable default of `48` for `N_SHARDS`, which serves to prevent data loss during times of heavy writes.

> #### Possible data loss during heavy writes
> KV writes are only guaranteed after ~1 sec since it is a distributed system, so we use sharding to split up keyword files. I am waiting for the Sync KV implementation for `workers-rs` to land, which should resolve this issue and `N_SHARDS` can be lowered.

Here's the following KV performance you can expect for each operation.
All commands execute at most one `list` command per invocation.

| Op | Performance |
|----|-------------|
| Get Keyword | `O(N_SHARDS)` |
| Write Document | `O(1 + kw_count)` |
| Update Document | `O(1 + new_keywords + old_keywords)`
| Search | `O(kw_count * N_SHARDS)`


As shown above, the `N_SHARDS` you choose significantly affects both the number of KV reads and writes you will make, but prevents data loss when inserting many documents at once.

# Deploy EdgeSearch

```bash
pnpm i
# Create a new wrangler configuration
pnpm run bin/edgesearch.js env init
# For managing wrangler.jsonc configs
eval $(pnpm run edgesearch env set)
pnpm run deploy
```

> ## Security
> I strongly recommend setting at least `API_KEY` in your `wrangler.jsonc` configuration, otherwise anyone can read your documents!

# API Usage

You can interact with EdgeSearch via its JSON API. First, identify your Cloudflare workers API endpoint, it should be something in the form of `edgesearch-api.username.workers.dev`.

We've included the `X-API-Key` header in the examples here for convenience, since the commands will still work without authentication enabled.

## Create an Index

First, let's create a new index called `sample` to store document and keyword data:

```bash
curl -X POST -H "X-API-Key: " https://edgesearch.username.workers.dev/sample
```

## Submit a Document

Submitting a document will automatically run language detection and keyword processing.  You can upload a file of any byte data you want. Keyword data is derived from the YAKE algorithm.

Documents stored are stored forever, and immediately accessible at the `document_id` returned. Now, let's index a new document on the `sample` index we just created.

```bash
curl -X POST -H "X-API-Key: " -d 'document body goes here' \
  https://edgesearch.username.workers.dev/sample/doc
```

Will return:
```json
{"id":"ysseRtTLpmEBsVEd","rev":1,"lang":"EN","body":"document body goes here","keywords":[["document body",0.9505961599793439],["document",0.8416830712200131],["body",0.7026344174397854]]}
```

> ### Documents with Custom IDs
> You can also create a document at a specific ID, if you need determinability.
> 
> ```bash
> curl -X POST -H "X-API-Key: " -d 'document body goes here' \
> https://edgesearch.username.workers.dev/sample/doc/abc123
> ```
> Will return:
> ```json
> {"id":"abc123","rev":1,"lang":"EN","body":"document body goes here","keywords":[...]}
> ```
>
> Attempting to create a document with an ID that already exists will return a HTTP failure.

> Nice Features To Do:
>  - [ ] Improved JSON processing
>  - [ ] Improved HTML processing
>  - [ ] Improved Binary processing
>

## Retrieve a Document

```bash
curl -X GET -H "X-API-Key: " \
  https://edgesearch.username.workers.dev/sample/doc/ysseRtTLpmEBsVEd
```

Fetching a document returns the same result as creating a document:
```json
{"id":"ysseRtTLpmEBsVEd","rev":1,"lang":"EN","body":"document body goes here","keywords":[["document body",0.9505961599793439],["document",0.8416830712200131],["body",0.7026344174397854]]}
```

## Searching

Queries can be complex, and negation works properly.

> The number of keywords in your search scales the number of KV reads that will occur.
> Searching a keyword requires reading all of the available shards for each keyword.
>
> We run a Durable Object called `DurableReader` that allows us to bypass the 1k KV op limit, by splitting key lookups into individual requests.
>
> This means you should be able to do some insane queries and have it fetch all the keyword scoring data in a distributed manner.

Some examples:

```rust
// Find documents with "a" AND "b" then exclude docs containing "c"
"a" && "b" && ~"c"
```

```rust
// Find documents with "ocean" and exclude any with "storm", "weather", or "tropical"
~("storm" || "weather" || "tropical") && "ocean"
```

### Limitations

You cannot do a simple negation of the entire document set. For example, the query `~"word"` will return no document results. You must first select documents with a positive keyword search before attempting to exclude them.

## Delete a document
Deletes a document from the KV store, and update any related keyword indexes.

```bash
curl -X DELETE -H 'X-API-Key: ' \
  https://edgesearch.username.workers.dev/sample/doc/ysseRtTLpmEBsVEd
```

The document will be deleted from the KV store, and any associated keyword data will be updated so the document no longer appears in search results.

## List Indexes
Display a list of all available indexes in the KV store.

```bash
curl -X GET -H 'X-API-Key: ' \
  https://edgesearch.username.workers.dev/indexes
```


# Configuration

EdgeSearch is directly configured through Cloudflare Worker environment values. The following configuration values currently exist:

| Variable | Default | Comment |
|---|---|---|
| `N_SHARDS` | 48 | The maximum number of keyword data shards that can exist. |
| `API_KEY` | _None_ | Set this to any value to require the `X-API-Key` header during requests. |
| `YAKE_NGRAMS` | 3 | The maximum number of words that can be in a keyword. |
| `YAKE_MINIMUM_CHARS` | 2 | The minimum number of characters in a keyword. |

### `N_SHARDS`
> Due to the latency required for maintaining synchronicity in a system with datacenters all over the globe, currently Cloudflare only promises KV data is written and distributed after ~1sec.
>
> I'm still waiting for the Sync KV feature to land in `workers-rs`, and then writes will become much more reliable.

EdgeSearch attempts to solve this solution with a simple hash in the form `hash(doc_id) % N_SHARDS`.

For a better understanding of how to optimize `N_SHARDS`:
  * `N_SHARDS = 2` - More data loss on heavy writes, much faster reads, accepts longer queries
  * `N_SHARDS = 48` - More balanced, more KV reads, reduced chance of data loss
  * `N_SHARDS = 128` - Excessive, limits search keywords, write conflicts if you're unlucky


# License

This project is licensed under the [MIT license](./LICENSE).