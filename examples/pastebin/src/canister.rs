use std::collections::HashMap;

use ic_kit::prelude::*;

const INDEX_HTML: &str = r#"
<!DOCTYPE html>
<html>
    <head>
        <meta charset="utf-8">
        <title>IC Pastebin</title>
    </head>
    <body>
        <h1>IC Pastebin</h1>
        <form action="/paste" method="POST">
            <textarea name="paste" rows="10" cols="80"></textarea>
            <br>
            <input type="submit" value="Submit">
        </form>
    </body>
</html>
"#;

#[get(route = "/")]
fn index_handler(_: HttpRequest, _: Params) -> HttpResponse {
    HttpResponse {
        status_code: 200,
        headers: vec![],
        body: INDEX_HTML.into(),
        streaming_strategy: None,
        upgrade: false,
    }
}

#[get(route = "/:file")]
fn get_file(_: HttpRequest, p: Params) -> HttpResponse {
    let file = p.get("file").unwrap();

    let res = format!("reading file: {}", file);

    HttpResponse {
        status_code: 200,
        headers: vec![],
        body: res.into(),
        streaming_strategy: None,
        upgrade: false,
    }
}

// #[put(route = "/:file")]
// fn put_file(req: HttpRequest, p: Params) -> HttpResponse {
//     let file = p.get("file").unwrap();

//     let res = format!("recieved file: {} ({} bytes)", file, req.body.len());

//     HttpResponse {
//         status_code: 200,
//         headers: vec![],
//         body: res.into_bytes(),
//         streaming_strategy: None,
//         upgrade: false,
//     }
// }

#[derive(KitCanister)]
#[candid_path("candid.did")]
pub struct PastebinCanister;
