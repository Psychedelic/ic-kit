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

const INDEX_MANPAGE: &str = r#"
IC PASTEBIN(1)                      IC PASTEBIN                       IC PASTEBIN(1)

NAME

        ic-pastebin - HTTP pastebin canister for the Internet Computer 

DESCRIPTION

        The ic-pastebin canister is a simple pastebin canister that allows users to
        upload text and get a URL to share it with others.

        The canister is written in Rust and uses the ic-kit library to provide
        access to the Internet Computer.

USAGE

        curl -T file.txt https://rrkah-fqaaa-aaaaa-aaaaq-cai.raw.ic0.app
        curl https://rrkah-fqaaa-aaaaa-aaaaq-cai.raw.ic0.app/file.txt
"#;

/// Index handler
#[get(route = "/")]
fn index_handler(r: HttpRequest, _: Params) -> HttpResponse {
    if let Some(ua) = r.header("User-Agent") {
        if ua.contains("curl") {
            return HttpResponse::ok().with_body(INDEX_MANPAGE.into());
        }
    }

    HttpResponse::ok().with_body(INDEX_HTML.into())
}

/// Get paste handler
#[get(route = "/:file")]
fn get_file(_: HttpRequest, p: Params) -> HttpResponse {
    let file = p.get("file").unwrap();
    ic::with(|data: &Data| match data.get(file) {
        Some(content) => HttpResponse::ok().with_body(content.clone()),
        None => HttpResponse::new(404).with_body(format!("404: file not found `{}`", file).into()),
    })
}

/// Upload paste handler
#[put(route = "/:file", upgrade = true)]
fn put_file(req: HttpRequest, p: Params) -> HttpResponse {
    let filename = p.get("file").unwrap();
    let res = format!("recieved file: {} ({} bytes)", filename, req.body.len(),);

    ic::with_mut(|d: &mut Data| {
        d.insert(filename.to_string(), req.body);
    });

    HttpResponse::ok().with_body(res.into())
}

#[derive(KitCanister)]
#[candid_path("candid.did")]
pub struct PastebinCanister;
