# Password hashing

Password hashing is a technique for storing and validating user passwords without exposing what those passwords are. All modern web apps must use this technique
to store credentials, and Rwf comes with built-in support for hashing using [Argon2](https://en.wikipedia.org/wiki/Argon2).

## Generate hashes

A password hash can be generated using the `rwf::crypto::hash` function, for example:

```rust
use rwf::crypto::hash;

let hashed = hash("secret_password".as_bytes()).unwrap();
```

The hash generated by this function is a Rust string; it can be saved in a database. Since Argon2 is cryptographically secure, strong passwords are reasonably protected against brute force attacks in case the hashes are leaked.

!!! note
    While hashes are hard to brute force, it's still inadvisable to allow hashes
    to be easily accessible to anyone. Make every effort to protect your production database
    against unauthorized access.

## Validate passwords

Hashes are used to check that some information the application has seen previously matches what the it's seeing now. For example, when one of your users wants to log into the application,
they will provide the application with a login and a password. The password can be validated against an existing hash, and if the two match, it's safe to assume that the password is correct.

Passwords can be validated using the `rwf::crypto::hash_validate` function, for example:

```rust
use rwf::crypto::hash_validate;

let matches = hash_validate(
    "secret_password".as_bytes(),
    &hashed,
).unwrap();
```

## Using with Tokio

You'll note that both `hash` and `hash_validate` functions are slow. In fact, it can take upwards a second to generate or validate a hash. This is done on purpose, to make hashes hard to brute force.
To avoid blocking the Tokio runtime and slowing down your application, make sure to use both functions inside blocking tasks:

```rust
use tokio::task::spawn_blocking;

let hashed = spawn_blocking(move || {
    hash("secret_password".as_bytes())
})
.await
.unwrap()
.unwrap();
```

## Learn more

- [examples/users](https://github.com/levkk/rwf/tree/main/examples/users)
