---
description: "ship a release"
user-invocable: true
disable-model-invocation: true
argument-hint: "[commit message]"
allowed-tools:
 - any
---
Perform a shipment of the ragent project by following these steps **in order**:

If any step fails, halt and report the problem:.

1. **execute all the steps in the 'simplify' skill with the 'all' argument**
2. execute all the steps in the 'docupdate' skill
3. execute all the steps in the 'rust-hygiene' skill
4. execute all the 'release' skill with the [commit message] as an argument
