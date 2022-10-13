# IC Kit Examples

This directory contains some example canister's implemented using the IC Kit. Each example tries
to be as simple as possible to demonstrate one aspect of the IC Kit and a possible design pattern
you can also use to develop canisters.

Hello World:

- [Hello](hello/): A simple canister that returns a greeting.

Simple State Manipulation:

- [Counter](counter/): A simple canister that allows incrementing and decrementing a counter.

Inter Canister Communication:

- [Multi Counter](multi_counter/): A canister that allows incrementing and decrementing a counter
  that is stored in separate canisters.

Child Canister Creation:

- [Factory Counter](factory_counter/): A canister that allows incrementing and decrementing a counter
  that is stored in separate canisters that are created on demand.

Inbound HTTP Server:

- [Pastebin](pastebin/): A canister that allows storing and retrieving unencrypted text snippets through http routing.
  Also features a simple canister generated frontend, that serves plaintext or html depending on the request client.
