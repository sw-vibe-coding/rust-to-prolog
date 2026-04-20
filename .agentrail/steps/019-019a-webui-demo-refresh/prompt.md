Refresh the web UI demos and add file-upload support.

Three concrete changes:

1. Add examples/hello.pl — a bareword Hello-World demo (write(hello_world), nl). Alphabetize it in web-ui/src/demos.rs (it lands between fib and liar in the dropdown).

2. Add an Upload .pl button to web-ui/src/lib.rs. Clicking it opens a file picker; selecting a .pl file reads its contents client-side via FileReader and replaces the source textarea contents. No server round-trip — everything stays in the WASM bundle.

3. Update docs/demos.md and the README's demo lists to include hello.pl and describe the upload flow.

Also add a reg-rs baseline r2p_hello.{rgt,out} so the CLI suite covers the new demo, and rebuild web-ui/pages/ with the new bundle.

Commit as 'webui: hello demo + upload .pl button'.