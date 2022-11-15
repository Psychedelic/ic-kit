# IC Pastebin

This directory contains an example HTTP canister implemented using the IC Kit. It is a simple HTTP pastebin that allows you to store and retrieve text.

## How to use

1. Build and deploy the canister:
    ```bash
    dfx deploy pastebin
    ```
2. View the canister's HTML UI at `http://rrkah-fqaaa-aaaaa-aaaaq-cai.localhost:8000/`
3. View the canister's manpage:
    ```bash
    curl rrkah-fqaaa-aaaaa-aaaaq-cai.localhost:8000
    ```
5. upload some text:
    ```bash
    curl -T file.txt rrkah-fqaaa-aaaaa-aaaaq-cai.localhost:8000
    ```
5. download some text:
    ```bash
    curl rrkah-fqaaa-aaaaa-aaaaq-cai.localhost:8000/file.txt
    ```