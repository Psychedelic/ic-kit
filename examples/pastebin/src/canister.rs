use ic_kit::prelude::*;
use std::collections::HashMap;

pub type Data = HashMap<String, Vec<u8>>;

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
    ic::with(|data: &Data| match data.get(file) {
        Some(content) => HttpResponse {
            status_code: 200,
            headers: vec![],
            body: content.clone(),
            streaming_strategy: None,
            upgrade: false,
        },
        None => HttpResponse {
            status_code: 404,
            headers: vec![],
            body: format!("404: file not found `{}`", file).into(),
            streaming_strategy: None,
            upgrade: false,
        },
    })
}

#[put(route = "/:file", upgrade = true)]
fn put_file(req: HttpRequest, p: Params) -> HttpResponse {
    let filename = p.get("file").unwrap();
    let res = format!("recieved file: {} ({} bytes)", filename, req.body.len(),);

    ic::with_mut(|d: &mut Data| {
        d.insert(filename.to_string(), req.body);
    });

    HttpResponse {
        status_code: 200,
        headers: vec![],
        body: res.into_bytes(),
        streaming_strategy: None,
        upgrade: false,
    }
}

#[derive(KitCanister)]
#[candid_path("candid.did")]
pub struct PastebinCanister;
