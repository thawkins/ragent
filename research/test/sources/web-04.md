# Web source

- URL: https://blog.logrocket.com/panic-vs-error-rust
- Title: [
- Captured (UTC): 2026-06-29T16:20:15.003475559+00:00

```text
[

###### Advisory boards aren’t only for executives. Join the LogRocket Content Advisory Board today →

][1]
[ [LogRocket blog logo] ][2]
* [Blog][3]
  * [Dev][4]
  * [Product Management][5]
  * [UX Design][6]
  * [Podcast][7]
  * [Product Leadership][8]
* [Features][9]
* [Solutions][10]
  * [Solve User-Reported Issues][11]
  * [Surface User Struggle][12]
  * [Optimize Conversion and Adoption][13]
* [Start Monitoring for Free][14]
* [Sign In][15]
2023-01-10
2519
#rust
Austin Roy Omondi
154293
104
Jan 10, 2023 ⋅ 8 min read

# panic! vs. error in Rust

[Austin Roy Omondi][16] Live long and prosper 👌
Table of contents
* [Table of contents][17]
* [Error handling: Rust vs. JavaScript][18]
* [Errors in Rust][19]
* [`Result` enum and recoverable errors][20]
* [Matching on different errors][21]
* [`Result <T,E>` type helper methods][22]
* [`panic!` macro and unrecoverable errors][23]
  * [How to trigger a `panic!`][24]
* [Recover or stop execution][25]
* [Viewing the stack trace or backtrace][26]
* [Summary][27]
[LogRocket Galileo logo]
Introducing Galileo AI
LogRocket’s Galileo AI watches every session, surfacing impactful user struggle and key behavior patterns.
[LEARN MORE][28]

## See how LogRocket's Galileo AI surfaces the most severe issues for you

### No signup required

Check it out

As every developer knows, errors tend to happen more often than not during application development. Error handling is a
fundamental concept in most programming languages that helps developers find and address errors in a program before it
is shipped to end users, thereby improving the UX.

[Rust Panic Vs Error]

Thankfully, Rust is known for its commitment to reliability and support for error handling. In this article, we’ll
explore how Rust enables developers to find and handle errors in their code, clarifying the difference between a
`panic!` and an error. Let’s get started!

## Table of contents
* [Error handling: Rust vs. JavaScript][29]
* [Errors in Rust][30]
* [`Result` enum and recoverable errors][31]
* [Matching on different errors][32]
* [`Result <T,E>` type helper methods][33]
* [`panic!` macro and unrecoverable errors][34]
  * [How to trigger a `panic!`][35]
* [Recover or stop execution][36]
* [Viewing the stack trace or backtrace][37]

## Error handling: Rust vs. JavaScript

Throwing errors is common in other programming languages, meaning that when there is an error in the code, the
subsequent processes [won’t be executed until they are handled correctly via `try...catch`][38].

However, in Rust, there is [no such thing as `try...catch`][39]. For context, we’ll review an example of how other
programming languages typically handle errors using `try...catch` and later show how Rust would distinguish a `panic!`
from an error with another example:

async function insertLaptop({ 
  storageSize,  
  model, 
  ram, 
  isRefurbished 
  }) { 
try { 
  if (isRefurbished) { 
    throw new Error("Cannot insert a Refurbished laptop"); 
  } 
const result = await db.laptop.add({ storage, model,ram }); 
return { success: true, id: result.id }; 
} catch(error) { 
return { success: false, id: undefined }; 
  }  
} 

In the JavaScript code snippet above, the `insertLaptop` function adds a new instance of a laptop to the database as
long as the laptop is not refurbished. The function is wrapped with a `try...catch` error-handling mechanism. The `try`
statement allows you to define a block of code to be tested for errors while it is being executed, whereas the `catch`
statement allows you to define a block of code to be executed if an error occurs in the `try` block.

When an error occurs, JavaScript will normally stop and generate an error message with some information about what
caused the error. The technical term for this is that JavaScript will throw an exception. This prevents the code from
completely crashing in the event that there is an error. This is the common practice among most programming languages.
However, in Rust, `panic!` and errors handle such exceptions.

## Errors in Rust

To better understand errors in Rust, we need to understand how Rust groups its errors. Unlike most programming languages
that do not distinguish between types of errors, Rust groups errors into two major categories, recoverable and
unrecoverable errors. Additionally, Rust does not have exceptions. It has the type Result `<T,``E>` for recoverable
errors and the `panic!` macro that stops further execution when the program encounters an unrecoverable error.

## `Result` enum and recoverable errors

In Rust, most errors fall into the recoverable category. [Recoverable errors][40] are not serious enough to prompt the
program to stop or fail abruptly since they can be easily interpreted and therefore handled or corrected. For
recoverable errors, one should report the error to the user to retry the operation or offer the user an alternative
course of action.

A good example is when a user inputs an invalid email. The error message might be something along the lines of `please
insert a valid email address and retry`. The `Result` type or `Result` enum is typically used in Rust functions to
return an `Ok` success value or an error `Err` value, as seen in the following example:

enum Result<T,E> { 
    OK(T), 
    Err(E) 
} 

`T` and `E` are generic type parameters. `Ok(T)` represents the return value when the case is successful and contains a
value, whereas `Err(E)` represents an error case and returns an error value. It’s important to note that we return
`Result` from functions when the errors are expected and recoverable.

Because `Result` has these generic type parameters, we can use the `Result` type and the functions defined on it in many
different situations where the successful value and error value we want to return may differ:

use std::fs::File; 
fn main() { 
    let f = File::open("main.jpg");  
    //this file does not exist 
    println!("{:?}",f); 
} 

Err(Error { repr: Os { code: 2, message: "No such file or directory" } }) 

The code above is a simple program to open a specified file. The program returns `OK(File)` if the file already exists
and `Err` if the file is not found. For this error and other types of errors where the value type is `Result <T,``E>`,
there is a recoverable error. It allows us to access the error value without having to break the whole program.

## Matching on different errors

Let’s revisit the previous code snippet. The code will `panic!` regardless of why `File::open` failed. To better debug
our program, we may need to take [different steps depending on the reason for failure][41]. For example, if the code
fails due to a non-existent file, we can create the file and return the new one.

If `File::open` fails for another reason, such as a user lacking permission to open the specified file, we want the code
to `panic!`, as it did in the previous code snippet. To achieve this result, we add an inner `match` expression:

use std::fs::File;
use std::io::ErrorKind;

fn main() {
    let logo_image = File::open("main.jpg");

    let logo_file = match logo_image {
        Ok(file) => file,
        Err(error) => match error.kind() {
            ErrorKind::NotFound => match File::create("main.jpg") {
                Ok(fc) => fc,
                Err(e) => panic!("We encountered a problem creating this file: {:?}", e),},
            other_error => {
                panic!("There was a problem opening the specified file: {:?}", other_error);
            }
        },
    };
}

In the code snippet above, we import `io::ErrorKind`, which contains `io::Error`, a [struct][42] from Rust’s standard
library. `io::Error` is the type of value returned by `File::open` inside the `Err` variant and has a method called
`kind`. It contains several variants that represent the different kinds of errors that might result from an `io`
operation. In our case, we get `ErrorKind::NotFound`, which implies that the file we’re trying to open does not exist.

The function also has an inner `match` expression on `error.kind()`. We are checking the condition, meaning whether the
value returned by `error.kind()` is the `NotFound` variant of the `ErrorKind` enum. If this condition is met, a new file
called `main.jpg` is created using `File::create`. `File::create` can also fail, so we include another option for the
inner `match` expression.

If `File::create` fails, we print a different error message. The second option of the outer `match` stays the same,
implying that the program will panic when it encounters any error besides the `NotFound` variant of the `ErrorKind`
enum.

## `Result <T,E>` type helper methods

There are other `Result <T, E>` type [helper methods][43] that you can use to communicate intent better than `match`.

### Rust `unwrap`

The `unwrap` method is better suited for more specific tasks. In our previous example, we can use `unwrap`. If the value
is an `Ok` variant, `unwrap` will return the value inside the `Ok`. If the `Result` is the `Err` variant, `unwrap` will
call the `panic!` macro for us. We’ll use our example from above to show `unwrap` in action:

use std::fs::File;

fn main() {
    let logo_image = File::open("main.jpg").unwrap();
}

If we run this code without a `main.jpg` file, we encounter an error message from the `panic!` call that the `unwrap`
method makes:

thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: Os {
code: 2, kind: NotFound, message: "No such file or directory" }',
src/main.rs:4:49

### Rust `expect`

`expect` is another helper method that works like `unwrap` but makes it possible to customize the `panic!` error
message. Let’s see `expect` in action:

use std::fs::File;

fn main() {
    let logo_file = File::open("main.jpg")
        .expect("main.jpg file should be included in the project for the program to run");
}

In the code above, we used `expect` instead of `unwrap`. It returns the file handles or calls the `panic!` macro in case
of an error. The difference is that the error message will be the parameter passed to `expect` as opposed to the default
`panic!` message that `unwrap` uses.

Let’s look at the output in case we call a non-existent `main.jpg` file:

thread 'main' panicked at 'main.jpg file should be included in the project for the 
program to run: Os {
code: 2, kind: NotFound, message: "No such file or directory" }',
src/main.rs:5:10

## `panic!` macro and unrecoverable errors

Unrecoverable errors, which cannot be handled, are most often the result of a bug. When they occur, unrecoverable errors
cause the program to abruptly fail. Additionally, the program cannot revert to its normal state, retry the failed
operation, or undo the error. An example of an unrecoverable error is trying to access a location beyond the end of an
`array x`.

In Rust, a `panic!` is a [macro][44] that terminates the program when it comes across an unrecoverable error. In simpler
terms, `panic!` is referring to a critical error. The `panic!` macro allows a program to terminate immediately and
provides feedback to the caller of the program.

### How to trigger a `panic!`

There are two ways to trigger a `panic!`. The first option is to manually `panic!` a Rust program. The second is by
taking an action that causes the code to `panic!`.

[

## Over 200k developers use LogRocket to create better digital experiences

Learn more →
][45]

To manually `panic!` the Rust program, we use the `panic!` macro. Invoke it as if you were triggering a function:

fn main() { 
panic!("panic vs. error in rust"); // because of panic, the program will terminate 
//immediately.  
println!("I am the unreachable statement ");  
} 

In the example above, we’ve included a `panic!` Macro. Once the program encounters it, it terminates immediately:

--&gt; hello.rs:3:5 
| 
2 | panic!("panic vs. error in rust");// because of panic, the program will terminate immediately.  
| --------------------------------- any code following this expression is unreachable 
3 | println!("I am the unreachable statement ");  
| ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ unreachable statement 
| 
= note: `#[warn(unreachable_code)]` on by default 
= note: this warning originates in the macro `println` (in Nightly builds, run with -Z macro-backtrace for more info) 
warning: 1 warning emitted 

Below is a conditional statement that invokes the `panic!` macro:

fn main() { 
  let no = 112;  
  //try with no above 100 and below 100  
  if no>=100 { 
  println!("Thank you for inserting your number, Registration is successful "); 
  } else { 
  panic!("NUMBER_TOO_SHORT");  
  } 
  println!("End of main"); 
} 

Below is the output when the variable is less than 100:

thread 'main' panicked at 'NUMBER_TOO_SHORT', hello.rs:7:8 
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace 

The output below is when the variable is greater than or equal to 100:

Thank you for inserting your number, Registration is successful 
End of main 

The example below shows an instance of the `panic!` call coming directly from a library as a result of a bug as opposed
to being manually called. The code below attempts to access an index in a vector. The error is due to the vector being
beyond the range of valid indexes:

fn main() { 
let f = vec![1, 2, 3]; 
v[40]; 
} 

The program is attempting to access the 41st element of our vector ( the vector at index 40), but the vector has only
three elements. In this situation, Rust will `panic!`. Using `[]` is supposed to return an element, but if you pass an
invalid index, there’s no element that Rust could return here that would be correct:

thread 'main' panicked at 'index out of bounds: the len is 3 but the index is 40', hello.rs:15:5 
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace  

## Recover or stop execution

When deciding whether your code should `panic!`, we need to consider the possibility of our code ending up in a bad
state. A [bad state][46] is when some assumption, guarantee, contract, or invariant has been broken, such as when
invalid values, contradictory values, or missing values are passed to your code, plus one or more of the following:
* The bad state is something that is unexpected, as opposed to something that will likely happen occasionally, like a
  user entering data in the wrong format
* Your code after this point needs to rely on not being in this bad state, rather than checking for the problem at every
  step
* There’s not a good way to encode this information in the types you use

When a user passes in values to our code that don’t make sense, it’s advisable to return an error where possible. This
gives the user the option of deciding what should be their next course of action.

However, in some cases, continuing could prove harmful or insecure. In such cases, it’s best to call `panic!` and alert
the user that there is a bug, prompting them to fix it during the development stage.

Another appropriate case to call `panic!` would be when you’re calling external code that is out of your control and it
returns an invalid state that you have no way of fixing. However, it’s more appropriate to return a `Result` than to
make a `panic!` when failure is expected.

## Viewing the stack trace or backtrace

Similar to other programming languages, `panic!` provides a stack trace or backtrace
of an error. However, the backtrace is not displayed on the terminal unless the `RUST_BACKTRACE` environment variable is
set to a value different than zero. Therefore, if you execute the following command statement in the previous code
example:

RUST_BACKTRACE=1 ./hello  

### Stack trace

RUST_BACKTRACE=1 cargo run 
thread 'main' panicked at 'index out of bounds: the len is 3 but the index is 40', hello.rs:15:5 
stack backtrace: 
0: rust_begin_unwind 
at /rustc/a55dd71d5fb0ec5a6a3a9e8c27b2127ba491ce52/library/std/src/panicking.rs:584:5 
1: core::panicking::panic_fmt 
at /rustc/a55dd71d5fb0ec5a6a3a9e8c27b2127ba491ce52/library/core/src/panicking.rs:142:14 
2: core::panicking::panic_bounds_check 
at /rustc/a55dd71d5fb0ec5a6a3a9e8c27b2127ba491ce52/library/core/src/panicking.rs:84:5 
3: <usize as core::slice::index::SliceIndex<[T]>>::index 
4: core::slice::index::<impl core::ops::index::Index<I> for [T]>::index 
5: <alloc::vec::Vec<T,A> as core::ops::index::Index<I>>::index 
6: hello::main 
7: core::ops::function::FnOnce::call_once 
note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace. 

## Summary

All in all, Rust has two kinds of errors, an error value returned from the `Result` type, and an error generated from
triggering the `panic!` macro. The `Result` type returns a recoverable error, or in other words, an error that doesn’t
cause the program to terminate.

On the other hand, the `panic!` macro triggers an error that is unrecoverable or that causes the Rust program to
terminate immediately.

## [LogRocket][47]: Full visibility into web frontends for Rust apps

Debugging Rust applications can be difficult, especially when users experience issues that are hard to reproduce. If
you’re interested in monitoring and tracking the performance of your Rust apps, automatically surfacing errors, and
tracking slow network requests and load time, [try LogRocket][48].

[LogRocket][49] lets you replay user sessions, eliminating guesswork around why bugs happen by showing exactly what
users experienced. It captures console logs, errors, network requests, and pixel-perfect DOM recordings — compatible
with all frameworks.

[LogRocket's Galileo AI][50] watches sessions for you, instantly identifying and explaining user struggles with
automated monitoring of your entire product experience.

[ [LogRocket Dashboard Free Trial Banner] ][51]

Modernize how you debug your Rust apps — [start monitoring for free][52].
* [#rust][53]

## Stop guessing about your digital experience with LogRocket

[
Get started for free
][54]

#### Recent posts:

[
[TanStack Start RSC vs. Next.js RSC: Performance, DX, and production readiness]

#### TanStack Start RSC vs. Next.js RSC: Performance, DX, and production readiness

][55]

We built the same app in TanStack Start RSC and Next.js RSC. TanStack shipped 40% less JS and built 4x faster — but
Next.js is still the safer production bet.

[Chizaram Ken][56]
Jun 25, 2026 ⋅ 7 min read
[
[Frontend Wrapped H1 2026: The nine biggest frontend storylines]

#### Frontend Wrapped H1 2026: The nine biggest storylines so far

][57]

From RSC vulnerabilities and the Vercel breach to TypeScript 7.0 Beta and AI agents — the nine frontend storylines that
defined H1 2026, ranked.

[Chizaram Ken][58]
Jun 23, 2026 ⋅ 9 min read
[
[Generating the feature with an AI coding assistant]

#### I shipped AI-generated React code: 4 bugs I fixed

][59]

AI tools generate working React code fast, but miss race conditions, empty states, debouncing, and accessibility. Here’s
how to catch bugs before production.

[Temitope Oyedele][60]
Jun 22, 2026 ⋅ 10 min read
[
[How to build a virtual engineering team with Gemini CLI subagents]

#### How to build a virtual engineering team with Gemini CLI subagents

][61]

Learn how to use Gemini CLI subagents to delegate frontend, backend, testing, and docs tasks to specialized agents with
guardrails and clear ownership.

[Emmanuel John][62]
Jun 18, 2026 ⋅ 10 min read
[View all posts][63]

Would you be interested in joining LogRocket's developer community?

Yea No Thanks

Join LogRocket’s Content Advisory Board. You’ll help inform the type of content we create and get access to exclusive
meetups, social accreditation, and swag.

[Sign up now][64]

[1]: https://lp.logrocket.com/blg/content-advisory-board-signup
[2]: https://logrocket.com
[3]: https://blog.logrocket.com/
[4]: https://blog.logrocket.com/dev
[5]: https://blog.logrocket.com/product-management
[6]: https://blog.logrocket.com/ux-design
[7]: https://podrocket.logrocket.com
[8]: https://stories.logrocket.com/
[9]: https://logrocket.com/features
[10]: #
[11]: https://logrocket.com/solutions/solve-user-issues
[12]: https://logrocket.com/solutions/surface-user-struggle
[13]: https://logrocket.com/solutions/optimize-conversion-adoption
[14]: https://app.logrocket.com/
[15]: https://app.logrocket.com/
[16]: https://blog.logrocket.com/author/austinroyomondi/
[17]: https://blog.logrocket.com/panic-vs-error-rust/#tableofcontents
[18]: https://blog.logrocket.com/panic-vs-error-rust/#error-handling-rust-vs-javascript
[19]: https://blog.logrocket.com/panic-vs-error-rust/#errors-rust
[20]: https://blog.logrocket.com/panic-vs-error-rust/#result-enum-recoverable-errors
[21]: https://blog.logrocket.com/panic-vs-error-rust/#matching-on-different-errors
[22]: https://blog.logrocket.com/panic-vs-error-rust/#result-t-e-type-helper-methods
[23]: https://blog.logrocket.com/panic-vs-error-rust/#panic-macro-unrecoverable-errors
[24]: https://blog.logrocket.com/panic-vs-error-rust/#how-trigger-panic
[25]: https://blog.logrocket.com/panic-vs-error-rust/#recover-stop-execution
[26]: https://blog.logrocket.com/panic-vs-error-rust/#view-stack-trace-backtrace
[27]: https://blog.logrocket.com/panic-vs-error-rust/#summary
[28]: https://logrocket.com/products/galileo-ai
[29]: #error-handling-rust-vs-javascript
[30]: #errors-rust
[31]: #result-enum-recoverable-errors
[32]: #matching-on-different-errors
[33]: #result-t-e-type-helper-methods
[34]: #panic-macro-unrecoverable-errors
[35]: #how-trigger-panic
[36]: #recover-stop-execution
[37]: #view-stack-trace-backtrace
[38]: https://blog.logrocket.com/exception-handling-in-javascript/
[39]: https://blog.logrocket.com/ditching-try-catch-and-null-checks-in-rust/
[40]: https://web.mit.edu/rust-lang_v1.25/arch/amd64_ubuntu1404/share/doc/rust/html/book/second-edition/ch09-02-recovera
ble-errors-with-result.html
[41]: https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html#matching-on-different-errors
[42]: https://doc.rust-lang.org/book/ch05-01-defining-structs.html
[43]: https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html#shortcuts-for-panic-on-error-unwrap-and
-expect
[44]: https://doc.rust-lang.org/book/ch19-06-macros.html
[45]: https://lp.logrocket.com/blg/learn-more
[46]: https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html#guidelines-for-error-handling
[47]: https://lp.logrocket.com/blg/rust-signup
[48]: https://lp.logrocket.com/blg/rust-signup
[49]: https://lp.logrocket.com/blg/rust-signup
[50]: https://lp.logrocket.com/blg/rust-signup
[51]: https://lp.logrocket.com/blg/rust-signup
[52]: https://lp.logrocket.com/blg/rust-signup
[53]: https://blog.logrocket.com/tag/rust/
[54]: https://lp.logrocket.com/blg/signup
[55]: https://blog.logrocket.com/tanstack-start-rsc-vs-next-js-rsc-performance-dx-production-readiness/
[56]: https://blog.logrocket.com/author/emmanuelodioko/
[57]: https://blog.logrocket.com/frontend-wrapped-h1-2026-the-nine-biggest-storylines/
[58]: https://blog.logrocket.com/author/emmanuelodioko/
[59]: https://blog.logrocket.com/generating-the-feature-with-an-ai-coding-assistant/
[60]: https://blog.logrocket.com/author/temitopeoyedele/
[61]: https://blog.logrocket.com/how-to-build-a-virtual-engineering-team-with-gemini-cli-subagents/
[62]: https://blog.logrocket.com/author/emmanueljohn/
[63]: https://blog.logrocket.com/
[64]: https://lp.logrocket.com/blg/content-advisory-board-signup
```
