# Web source

- URL: https://ddanilov.me/panic-in-rust
- Title: [Dmitry Danilov][1]
- Captured (UTC): 2026-06-29T16:20:21.904210074+00:00

```text
[Dmitry Danilov][1]
* [Tags][2]
* [Archive][3]
* [Talks][4]
* [About Me][5]
[ [Navigation bar avatar] ][6]

# Terminating with panic! in Rust

Posted on May 31, 2021 · 3 min read

The Rust programming language provides a few ways to terminate a program when it reaches an unrecoverable state by
calling the macro `std::panic!` - a reference to kernel panic that I have found quite amusing.
It comes in handy when an assert needs to used within code, such as for unit tests, and it is eventually called by the
method `unwrap` of the `Option` and `Result` enums.

From my experience as a C/C++ engineer (I hope C and C++ enthusiasts, as well as the almighty coding standard Gods, will
forgive me for this blasphemy of placing a slash between the two languages), `panic!` was initially a synonym of `abort`
in C and C++, but with a few more features, such as stack unwinding. The goal of this post is to shed some light on a
few of the differences between `panic!` and `abort` that I have personally encountered.

Let us start with a simple program that immediately ‘panics’ when it is run:

`

───────┬────────────────────────────────────────────────────────────────────────
1      │fn main() {                                                             
2      │    panic!("Panic in the main thread!");                                
3      │    println!("Hello, world!");                                          
4      │}                                                                       
───────┴────────────────────────────────────────────────────────────────────────
`

The program is terminated and the output reveals where the panic was triggered. As a bonus, the application can be
configured via an environment variable to show its backtrace (stack unwinding).

`$ cargo run
Hello, world!
thread 'main' panicked at 'Panic in the main thread!', src/main.rs:2:5
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
`

It becomes even more intriguing when the exit code of the process is checked right after termination:

`$ echo $?
101
`

Rust sets the exit code to `101` explicitly when a process panics by calling the `exit` function, while `abort` signals
the kernel to kill the process (a detailed explanation of how `abort` works on Unix systems can be found in an earlier
[post][7]). In practice, this means that no core dumps are generated in the default configuration.

Now, let us take a look at what happens when `panic!` is called from a sub-thread:

`

───────────────────┬────────────────────────────────────────────────────────────────────────────────────────────────────
1                  │use std::thread;                                                                                    
2                  │                                                                                                    
3                  │fn main() {                                                                                         
4                  │                                                                                                    
5                  │    let handle = thread::spawn( || {                                                                
6                  │        println!("Thread started!");                                                                
7                  │        panic!("Panic in a thread!");                                                               
8                  │    });                                                                                             
9                  │                                                                                                    
10                 │    handle.join();                                                                                  
11                 │                                                                                                    
12                 │    println!("Hello, world!");                                                                      
13                 │}                                                                                                   
───────────────────┴────────────────────────────────────────────────────────────────────────────────────────────────────
`

Output:

`$ cargo run
Thread started!
thread '<unnamed>' panicked at 'Panic in a thread!', src/main.rs:7:9
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
Hello, world!

$ echo $?
0
`

The output clearly states that the thread has panicked but the main thread continues running, even after calling `join`!
It can thus be concluded that `panic` does not exit the entire process, but rather only the current thread; this is
completely different from C’s `abort`!

My continued interest in the Rust language grows precisely due to features such as this, where the language provides
elegant methods for terminating a process in the case where a background thread crashes.

If we were to force an ultimatum on the result of `join`, the shortest way is to `unwrap` the return value:

`

─────┬─────────────────────────────
1    │...                          
2    │handle.join().unwrap();      
3    │...                          
─────┴─────────────────────────────
`

The result contains an error and unwrapping leads to panic in the main thread:

`$ cargo run
Thread started!
thread '<unnamed>' panicked at 'Panic in a thread!', src/main.rs:7:9
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: Any', src/main.rs:10:19
`

Another way to manipulate the output of `join` is to check the result and decide what to do during runtime; the
following example uses `match`:

`

───────┬────────────────────────────────────────────────────────────────────────────────
1      │match handle.join() {                                                           
2      │    Ok(_) => println!("Joined!"),                                               
3      │    Err(_) => println!("Join failed"),                                          
4      │};                                                                              
───────┴────────────────────────────────────────────────────────────────────────────────
`

Note that this example only prints the error and the program still exits with `0`.

But wait, there’s more! For those who are not big fans of change, Rust even provides the possibility to configure
`panic!` to call `abort`; this can be done via Cargo.toml in the project:

`

─────────┬────────────────────────────────────────────────────────
1        │[profile.dev]                                           
2        │panic = "abort"                                         
3        │                                                        
4        │[profile.release]                                       
5        │panic = "abort"                                         
─────────┴────────────────────────────────────────────────────────
`

The result is the same as calling `abort` in C: the application is terminated with `SIGABRT` and if the system is
configured, a core dump is generated:

`$ cargo run
Thread started!
thread '<unnamed>' panicked at 'Panic in a thread!', src/main.rs:7:9
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
[1]    67943 abort      cargo run
134
`

Rust’s flexibility truly does not cease to amaze and I will diligently continue to provide such examples which I believe
other enthusiasts should be aware of and use.

*Special thanks to [Rina Volovich][8] for editing.*

Please share your thoughts on [Twitter][9], or [LinkedIn][10].

Tags: [#rust][11] [#rustlang][12] [#panic][13] [#abort][14] [#coredump][15]
Share: [ Twitter ][16] [ Facebook ][17] [ LinkedIn ][18]

## See also
* [I Zeroized My Secret. Or Did I?][19]
* [Configuring core dumps in docker][20]
* [How to overwrite a file in Rust][21]
* [How signals are handled in a docker container][22]
* [← Previous Post][23]
* [Next Post →][24]
* [ GitHub ][25]
* [ Twitter ][26]
* [ LinkedIn ][27]
* [ StackOverflow ][28]
* [ RSS ][29]
* [ Telegram ][30]

Dmitry Danilov  •  2026  •  [ddanilov.me][31]

Powered by [Beautiful Jekyll][32]

[1]: https://ddanilov.me/
[2]: /tags
[3]: /posts
[4]: /talks
[5]: /aboutme
[6]: https://ddanilov.me/
[7]: /how-signals-are-handled-in-a-docker-container
[8]: https://www.linkedin.com/in/rina-volovich/
[9]: https://twitter.com/dbdanilov/status/1399435722441084931?s=20
[10]: https://www.linkedin.com/posts/ddanilov_terminating-with-panic-in-rust-dmitry-activity-6805200803901530112-zQGX?ut
m_source=share&utm_medium=member_desktop
[11]: /tags#rust
[12]: /tags#rustlang
[13]: /tags#panic
[14]: /tags#abort
[15]: /tags#coredump
[16]: https://twitter.com/intent/tweet?text=Terminating+with+panic%21+in+Rust&url=https%3A%2F%2Fddanilov.me%2Fpanic-in-r
ust
[17]: https://www.facebook.com/sharer/sharer.php?u=https%3A%2F%2Fddanilov.me%2Fpanic-in-rust
[18]: https://www.linkedin.com/shareArticle?mini=true&url=https%3A%2F%2Fddanilov.me%2Fpanic-in-rust
[19]: /zeroize
[20]: /how-to-configure-core-dump-in-docker-container
[21]: /how-to-overwrite-a-file-in-rust
[22]: /how-signals-are-handled-in-a-docker-container
[23]: /how-to-configure-core-dump-in-docker-container
[24]: /dockerized-cpp-build
[25]: https://github.com/f-squirrel
[26]: https://twitter.com/dbdanilov
[27]: https://linkedin.com/in/ddanilov
[28]: https://stackoverflow.com/users/1333734/fsquirrel
[29]: /feed.xml
[30]: https://t.me/ddanilov_me
[31]: https://ddanilov.me/
[32]: https://beautifuljekyll.com
```
