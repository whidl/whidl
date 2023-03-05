# WHiDL documentation

This documentation is deployed to https://whidl.github.io

## Building the docs

First you must build the whidl web assembly module.

```
wasm-pack build --target web
```

Then you can build the documentation using the commands below. The documentation
build directory is `docs/build`.

```
cd docs
npm install
npm run build
```

You can also run the documentation app from a development server.

```
cd docs
npm install
npm run start
```
