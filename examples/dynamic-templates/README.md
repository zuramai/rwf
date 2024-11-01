
# Dynamic templates

Rwf has its own template language, heavily inspired (if not shamelessly copied) from Rails' ERB.

## Quick example

If you've used Rails before, you'll find this syntax familiar:

```erb
<p><%= text %></p>

<ul>
<% for item in list %>
    <li><%= item.upcase %><li>
<% end %>
<ul>

<script>
<%- no_user_inputs_allowed_code %>
</script>

<% if value == "on" %>
    <p>Now you see me</p>
<% else %>
    <p>Now you don't</p>
<% end %>
```

## Operations

Rwf's templates syntax is very small and simple:

| Operation | Description |
|----------|-------------|
| `<%` | Code block start. |
| `%>` | Code block end. |
| `<%=` | Print the following expression value (don't forget to close the code block). |
| `<%-` | Print expression without escaping "dangerous" HTML characters. |
| `<% if expression %>` | If block which evaluates the expression for truthiness. |
| `<% elsif expression %>`| Else if block, works just like the if block. |
| `<% else %>` | Else block. |
| `<% for item in list %>` | For loop. |
| `<% end %>` | Indicates the end of an if statement or for loop. |
| `+`, `-`, `*`, `/`, `==`, `%` | Addition, subtraction, multiplication, division, equality, modulo. |

## Rendering templates

Templates can be rendered directly from a Rust string:

```rust
#[derive(rwf::macros::Context)]
struct Index {
    first_name: String,
    user_id: i64,
}

let template = Template::from_str("<p>Ahoy there, <%= first_name %>! (id: <%= user_id %></p>")?;
let context = Index { first_name: "Josh".into(), user_id: 1 };

let result = template.render(context.try_into()?)?;

assert_eq!(result, "<p>Ahoy there, Josh! (id: 1)</p>");
```

Templates can be placed in files anywhere the Rust program can access them:

```rust
let template = Template::load("templates/index.html")?;
let result = template.render(context.try_into()?)?;
```

`templates/index.html` is a path relative to current wording directory (`$PWD`).

Templates don't have to be HTML, and can be used to render any kind of files, e.g. plain text, CSS, JavaScript, etc.

## Passing values to templates

Rwf's templates support many data types, e.g. strings, integers, lists, hashes, and even models. For example, a list of users can be passed directly into a template:

```rust
let users = User::all()
    .fetch_all(&mut conn)
    .await?;

let template = Template::from_str(
"<ul>
    <% for user in users %>
        <li><%= user.email %></li>
    <% end %>
</ul>")?;

#[derive(rwf::macros::Context)]
struct Context {
    users: Vec<User>,
}

let context = Context { users };

let rendered = template.render(&context.try_into()?)?;
```

## Data types

Multiple Rust data types are supported out of the box and each data type comes with its own operations.

| Template data type | Rust data type |
|-----------|---------|
| Integer | `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64` |
| Float | `f32`, `f64` |
| String | `String`, `&str` |
| List | `Vec` with any Rust data type, including the ORM's models |
| Hash | `HashMap` of any Rust data type, including the ORM's models |

### Operations

Each template data type supports its own operations.

#### Number

| Operation | Description | Example |
|-----------|-------------|---------|
| `abs` | Get the absolute value (non-negative) | `<%= 25.abs %>` |
| `to_string`, `to_s` | Convert the number to a string | `<% if 25.to_s == "25" %>` |
| `to_f`, `to_float` | Convert the number to a floating point number | `<% if 25.to_f == 25.0 %>` |
| `times` | Create a list of numbers enumerated from 0 to the number | `<% for i in 25.times %>` |

#### Float

| Operation | Description | Example |
|-----------|-------------|---------|
| `abs` | Get the absolute value (non-negative) | `<%= -25.0.abs %>` |
| `ceil` | Ceil the floating point to the nearest integer | `<% if 25.5.ceil == 26 %>` |
| `floor` | Floor the floating point to the nearest integer | `<% if 25.5.ceil == 25 %>` |
| `round` | Round the floating point to the nearest integer | `<% if 25.5.ceil == 26 %>` |
| `to_string`, `to_s` | Convert the floating point to a string representation. | `<%= 25.5.to_s %>` |

#### String

| Operation | Description | Example |
|-----------|-------------|---------|
| `to_uppercase`, `upcase` | Convert the string to UPPERCASE | `<%= "hello".upcase %>` |
| `to_lowercase`, `downcase` | Convert the string to lowercase | `<%= "BYE".downcase %>` |
| `trim` | Trim new lines and spaces from both ends of the string | `<%= " hello ".trim %>` |

#### List

| Operation | Description | Example |
|-----------|-------------|---------|
| `enumerate` | Convert the list to a list of tuples, with the index and the value | `<% for item in list.enumerate  %> <%= item.0 %> <%= item.1 %> <% end %>` |
| `reverse`, `rev` | Create a new list by reading the list in reverse order (end to beginning) | `<% for item in list.rev %>` |
| `list.index` | Access the `index` element (starting at 0) | `<%= list.5 %>` |


#### Hash

| Operation | Description | Example |
|-----------|-------------|---------|
| `keys` | Convert the hash to a list of its keys | `<% for key in hash.keys  %> <%= key %> <% end %>` |
| `values` | | Convert the hash to a list of its values | `<% for value in hash.values  %> <%= value %> <% end %>` |
| `iter` | Convert the hash to a list of tuples, with the key and the value | `<% for pair in hash.iter  %> <%= pair.0 %> : <%= pair.1 %> <% end %>` |
| `hash.key` | Access the `key` element  | `<%= user.email %>` |


#### Globals

Rwf's templates have global functions which can be executed in any template and context.

| Function | Description | Example |
|-----------|-------------|---------|
| `encrypt_number` | Encrypt an integer using the secure id algorithm (AES-128) | `<%= encrypt_number(25) %>` |
| `decrypt_number` | Decrypt an integer encrypted with `encrypt_number` | `<%= decrypt_number(encrypted) %>` |
| `rwf_head` | Print the included `<head>` elements that make Rwf-powered frontends work, e.g. Turbo | `<head><%- rwf_head %></head>` |

## Caching templates

Reading templates from disk is usually quick, but compiling them can take some time. In development, they are compiled every time they are fetched, which allows to iterate on their contents quickly, while in production they are cached in memory for performance.

The caching behavior is controlled via configuration and requires no code modifications:

```toml
[general]
cache_templates = true
```
