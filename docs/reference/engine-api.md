# Engine API

Every operation of the ElectronicWerk engine is a **method** with a name and
JSON parameters. The desktop app, the [`ewerk` command-line tool](./cli) and
the automated tests all use this one API. A feature that exists in the engine is
therefore automatically scriptable.

## Request and reply

A request is a JSON object:

```json
{ "method": "engine.ping", "params": { "any": "value" } }
```

- `method` (string, required): the method name.
- `params` (any JSON value, optional): the parameters; `null` when omitted.
  Unknown fields in the request object are rejected.

The reply is either a result:

```json
{ "result": { "pong": true, "echo": { "any": "value" } } }
```

or an error:

```json
{ "error": { "code": "methodNotFound", "message": "unknown method `x`" } }
```

## Error codes

| Code             | Meaning                                                          |
| ---------------- | ---------------------------------------------------------------- |
| `invalidRequest` | The request is not valid JSON or does not match the format above |
| `methodNotFound` | There is no method with this name                                |
| `invalidParams`  | The parameters do not match what the method expects              |

The desktop app adds three codes for problems outside the engine:
`hostError` (the engine process reported an exception), `disconnected` (the
engine restarted while a request was pending) and `timeout` (no reply within
10 seconds).

## Methods

### `engine.info`

Returns information about the engine build.

| Field     | Type    | Description                                                             |
| --------- | ------- | ----------------------------------------------------------------------- |
| `name`    | string  | Always `ElectronicWerk Engine`                                          |
| `version` | string  | Engine version, for example `0.1.0`                                     |
| `os`      | string  | Operating system the engine was built for (`windows`, `macos`, `linux`) |
| `arch`    | string  | Processor architecture (`x86_64`, `aarch64`)                            |
| `threads` | number  | Number of worker threads the engine may use                             |
| `debug`   | boolean | `true` for development builds                                           |

```console
$ ewerk call engine.info
{"result":{"arch":"aarch64","debug":false,"name":"ElectronicWerk Engine","os":"macos","threads":8,"version":"0.1.0"}}
```

### `engine.methods`

Returns the names of all methods, sorted alphabetically.

```console
$ ewerk call engine.methods
{"result":["engine.info","engine.methods","engine.ping"]}
```

### `engine.ping`

Returns `{"pong": true, "echo": <params>}`. Useful to check that the engine is
reachable and that parameters arrive unchanged.

```console
$ ewerk call engine.ping '[1, 2, 3]'
{"result":{"echo":[1,2,3],"pong":true}}
```

## Versioning

Method names and result fields are stable within a major version. New methods
and new optional fields can appear in any release; they are listed in the
[changelog](https://github.com/Ujap1994/ElectronicWerk/blob/main/CHANGELOG.md).
