# Edgesearch

A document store with full-text search built to run on Cloudflare Workers and KV, with [workers-rs](https://github.com/cloudflare/workers-rs) and WebAssembly.

## Features
- No servers or databases to create, manage, or scale.
- Simple API key authentication (just set `API_KEY` in your Worker env)
- English keyword + language detection on document submission (more coming soon)
- Organize documents and keywords into unique indexes
- Simple query language, example: `("word" || "wordle") && ~"pop"`
- Look up all documents for an individual keyword (`/:index/keyword/:keyword`)
- Balances KV storage usage for fast search + document store.
- Runs fast [WASM](https://webassembly.org/) code at Cloudflare edge PoPs for low-latency requests.
- Designed to scale (Cloudflare KV has no limit)
- Options to balance between high-write scenarios and high-read scenarios. 
- [Hash table](https://en.wikipedia.org/wiki/Hash_table) sharding for minimizing data loss over slow KV replication

## Bonus Features
- Will definitely burn through your entire Cloudflare KV free plan in a day
- There is technically no limit to how many 2MB documents KV will store
- Searching is really fast
- Questionably written Rust (room for improvement)

## Drawbacks

> ### High-volume Writing
>
> Due to the latency required for maintaining synchronicity in a system with datacenters all over the globe, currently Cloudflare only promises KV data is written and distributed after ~1sec.
>
> EdgeSearch attempts to solve this solution with a simple hash shard based on the document's
> ID. We provide a configuration value via the `N_SHARDS` worker environment variable.
> 
> For a better understanding of how to optimize `N_SHARDS`:
>   * `N_SHARDS=2`
>     - An extremely conservative number for sharding
>     - If more than one write to a keyword file happens at a time, data may be overwritten
>     - Maximum of 2 KV entries for a single keyword
>   * `N_SHARDS=48`
>     - More reasonable
>     - Much reduced opportunity for overwriting data when there are more shards
>     - Maximum of 48 KV entries for a single keyword
>   * `N_SHARDS=512`
>     - Optimized for writing
>     - RIP your KV limits
>     - If you get a conflict you're unlucky :(
>
> 
> ### KV Usage
>
> As shown above, the sharding number you choose significantly affects the size of your actual KV storage used.
> By default, the application default is `48`, which I think is a reasonable balance to start with.

## How it works

Deploy the Rust WASM worker to your Cloudflare account and begin adding documents, searching, and querying via JSON over HTTPS.

### Create an Index

First, create a new index called `default` (or whichever index name you wish) that will store your 
document and keyword data:

```bash
curl -X POST https://edgesearch.yourname.workers.dev/default
```

### Submit a Document

Submitting a document will automatically run language detection and keyword processing.  You can upload a file of any byte data you want. Right now it will throw YAKE at it and see what comes out. What you put in it is up to you.

The document data will be stored indefinitely and is acessible at the `document_id` that is returned 
during creation. 

> Nice Features To Do:
>  - [ ] Improved JSON processing
>  - [ ] Improved HTML processing
>  - [ ] Improved Binary processing
>

Now, let's index a new document on the `default` index we just created.

```bash
curl -X POST -d 'lorem ipsum' https://edgesearch.yourname.workers.dev/default/doc
```

### Searching

Search terms have an associated mode. There are three modes that match documents in different ways:

|Mode|Document match condition|
|---|---|
|Require|Has all terms with this mode.|
|Contain|Has at least one term with this mode.|
|Exclude|Has none of the terms with this mode.|


#### Search Query Syntax
```rust
("a" && "b" && "c") && ("e" || "f" || "g") && ~("x" || "y" || "z")
```

Including any keyword will trigger a lookup for all keyword shards, allowing us to collect the full list of related documents and always be up to date.

> ```
> Reads = keyword_count * N_SHARDS
> Write = 0
> ```

## Usage

TODO

### Deploy the worker

```bash
pnpm i
pnpm run edgesearch env init
# For managing wrangler.jsonc configs
eval $(pnpm run edgesearch env set)
pnpm run deploy
```

### Testing locally

[edgesearch-test-server](./tester) loads a built worker to run locally.

This will create a local server on port 8080:

```bash
npx edgesearch-test-server \
  --output-dir /path/to/edgesearch/build/output/dir/ \
  --port 8080
```

The client can be used with a local test server; provide the origin (e.g. `http://localhost:8080`) to the constructor (see below).

### Calling the API

A JavaScript [client](./client/) for the browser and Node.js is available for using a deployed Edgesearch worker:

```typescript
import * as Edgesearch from 'edgesearch-client';

type Document = {
  title: string;
  artist: string;
  year: number;
};

const client = new Edgesearch.Client<Document>('https://my-edgesearch.me.workers.dev');
const query = new Edgesearch.Query();
query.add(Edgesearch.Mode.REQUIRE, 'world');
query.add(Edgesearch.Mode.CONTAIN, 'hello', 'welcome', 'greetings');
query.add(Edgesearch.Mode.EXCLUDE, 'bye', 'goodbye');
let response = await client.search(query);
query.setContinuation(response.continuation);
response = await client.search(query);
```
