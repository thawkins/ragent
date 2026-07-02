# Web source

- URL: https://github.com/rust-lang/rust/blob/master/library/std/src/panicking.rs
- Title: [Skip to content][1]
- Captured (UTC): 2026-06-29T16:20:23.796469801+00:00

```text
[Skip to content][1]

## Navigation Menu

Toggle navigation
[ Sign in ][2]
Appearance settings
* Platform
  * AI CODE CREATION
    * [
      GitHub CopilotWrite better code with AI
      ][3]
    * [
      GitHub Copilot appDirect agents from issue to merge
      ][4]
    * [
      MCP Registry^{New}Integrate external tools
      ][5]
  * DEVELOPER WORKFLOWS
    * [
      ActionsAutomate any workflow
      ][6]
    * [
      CodespacesInstant dev environments
      ][7]
    * [
      IssuesPlan and track work
      ][8]
    * [
      Code ReviewManage code changes
      ][9]
  * APPLICATION SECURITY
    * [
      GitHub Advanced SecurityFind and fix vulnerabilities
      ][10]
    * [
      Code securitySecure your code as you build
      ][11]
    * [
      Secret protectionStop leaks before they start
      ][12]
  * EXPLORE
    * [Why GitHub][13]
    * [Documentation][14]
    * [Blog][15]
    * [Changelog][16]
    * [Marketplace][17]
  [View all features][18]
* Solutions
  * BY COMPANY SIZE
    * [Enterprises][19]
    * [Small and medium teams][20]
    * [Startups][21]
    * [Nonprofits][22]
  * BY USE CASE
    * [App Modernization][23]
    * [DevSecOps][24]
    * [DevOps][25]
    * [CI/CD][26]
    * [View all use cases][27]
  * BY INDUSTRY
    * [Healthcare][28]
    * [Financial services][29]
    * [Manufacturing][30]
    * [Government][31]
    * [View all industries][32]
  [View all solutions][33]
* Resources
  * EXPLORE BY TOPIC
    * [AI][34]
    * [Software Development][35]
    * [DevOps][36]
    * [Security][37]
    * [View all topics][38]
  * EXPLORE BY TYPE
    * [Customer stories][39]
    * [Events & webinars][40]
    * [Ebooks & reports][41]
    * [Business insights][42]
    * [GitHub Skills][43]
  * SUPPORT & SERVICES
    * [Documentation][44]
    * [Customer support][45]
    * [Community forum][46]
    * [Trust center][47]
    * [Partners][48]
  [View all resources][49]
* Open Source
  * COMMUNITY
    * [
      GitHub SponsorsFund open source developers
      ][50]
  * PROGRAMS
    * [Security Lab][51]
    * [Maintainer Community][52]
    * [Accelerator][53]
    * [GitHub Stars][54]
    * [Archive Program][55]
  * REPOSITORIES
    * [Topics][56]
    * [Trending][57]
    * [Collections][58]
* Enterprise
  * ENTERPRISE SOLUTIONS
    * [
      Enterprise platformAI-powered developer platform
      ][59]
  * AVAILABLE ADD-ONS
    * [
      GitHub Advanced SecurityEnterprise-grade security features
      ][60]
    * [
      Copilot for BusinessEnterprise-grade AI features
      ][61]
    * [
      Premium SupportEnterprise-grade 24/7 support
      ][62]
* [Pricing][63]
Search or jump to...

# Search code, repositories, users, issues, pull requests...

Search
Clear
[Search syntax tips][64]

# Provide feedback

We read every piece of feedback, and take your input very seriously.

Include my email address so I can be contacted
Cancel Submit feedback

# Saved searches

## Use saved searches to filter your results more quickly

Name
Query

To see all available qualifiers, see our [documentation][65].

Cancel Create saved search
[ Sign in ][66]
[ Sign up ][67]
Appearance settings
Resetting focus
You signed in with another tab or window. [Reload][68] to refresh your session. You signed out in another tab or window.
[Reload][69] to refresh your session. You switched accounts on another tab or window. [Reload][70] to refresh your
session. Dismiss alert

### Uh oh!


There was an error while loading. [Please reload this page][71].

[ rust-lang ][72] / ** [rust][73] ** Public
* ### Uh oh!
  
  
  There was an error while loading. [Please reload this page][74].
* [ Notifications ][75] You must be signed in to change notification settings
* [ Fork 15k ][76]
* [ Star 114k ][77]
* [ Code ][78]
* [ Issues 5k+ ][79]
* [ Pull requests 1.2k ][80]
* [ Actions ][81]
* [ Projects ][82]
* [ Security and quality 6 ][83]
* [ Insights ][84]
Additional navigation options
* [ Code ][85]
* [ Issues ][86]
* [ Pull requests ][87]
* [ Actions ][88]
* [ Projects ][89]
* [ Security and quality ][90]
* [ Insights ][91]

## FilesExpand file tree

main

## Breadcrumbs
1. [rust][92]
2. /[library][93]
3. /[std][94]
4. /[src][95]

/

# panicking.rs

Copy path
BlameMore file actions
BlameMore file actions

## Latest commit

## History

[History][96]
[][97]History
895 lines (804 loc) · 31.3 KB
 main

## Breadcrumbs
1. [rust][98]
2. /[library][99]
3. /[std][100]
4. /[src][101]

/

# panicking.rs

Copy path
Top

## File metadata and controls
* Code
* Blame

895 lines (804 loc) · 31.3 KB
[Raw][102]
Copy raw file
Download raw file
Open symbols panel
Edit and raw actions
1
2
3
4
5
6
7
8
9
10
11
12
13
14
15
16
17
18
19
20
21
22
23
24
25
26
27
28
29
30
31
32
33
34
35
36
37
38
39
40
41
42
43
44
45
46
47
48
49
50
51
52
53
54
55
56
57
58
59
60
61
62
63
64
65
66
67
68
69
70
71
72
73
74
75
76
77
78
79
80
81
82
83
84
85
86
87
88
89
90
91
92
93
94
95
96
97
98
99
100
101
102
103
104
105
106
107
108
109
110
111
112
113
114
115
116
117
118
119
120
121
122
123
124
125
126
127
128
129
130
131
132
133
134
135
136
137
138
139
140
141
142
143
144
145
146
147
148
149
150
151
152
153
154
155
156
157
158
159
160
161
162
163
164
165
166
167
168
169
170
171
172
173
174
175
176
177
178
179
180
181
182
183
184
185
186
187
188
189
190
191
192
193
194
195
196
197
198
199
200
201
202
203
204
205
206
207
208
209
210
211
212
213
214
215
216
217
218
219
220
221
222
223
224
225
226
227
228
229
230
231
232
233
234
235
236
237
238
239
240
241
242
243
244
245
246
247
248
249
250
251
252
253
254
255
256
257
258
259
260
261
262
263
264
265
266
267
268
269
270
271
272
273
274
275
276
277
278
279
280
281
282
283
284
285
286
287
288
289
290
291
292
293
294
295
296
297
298
299
300
301
302
303
304
305
306
307
308
309
310
311
312
313
314
315
316
317
318
319
320
321
322
323
324
325
326
327
328
329
330
331
332
333
334
335
336
337
338
339
340
341
342
343
344
345
346
347
348
349
350
351
352
353
354
355
356
357
358
359
360
361
362
363
364
365
366
367
368
369
370
371
372
373
374
375
376
377
378
379
380
381
382
383
384
385
386
387
388
389
390
391
392
393
394
395
396
397
398
399
400
401
402
403
404
405
406
407
408
409
410
411
412
413
414
415
416
417
418
419
420
421
422
423
424
425
426
427
428
429
430
431
432
433
434
435
436
437
438
439
440
441
442
443
444
445
446
447
448
449
450
451
452
453
454
455
456
457
458
459
460
461
462
463
464
465
466
467
468
469
470
471
472
473
474
475
476
477
478
479
480
481
482
483
484
485
486
487
488
489
490
491
492
493
494
495
496
497
498
499
500
501
502
503
504
505
506
507
508
509
510
511
512
513
514
515
516
517
518
519
520
521
522
523
524
525
526
527
528
529
530
531
532
533
534
535
536
537
538
539
540
541
542
543
544
545
546
547
548
549
550
551
552
553
554
555
556
557
558
559
560
561
562
563
564
565
566
567
568
569
570
571
572
573
574
575
576
577
578
579
580
581
582
583
584
585
586
587
588
589
590
591
592
593
594
595
596
597
598
599
600
601
602
603
604
605
606
607
608
609
610
611
612
613
614
615
616
617
618
619
620
621
622
623
624
625
626
627
628
629
630
631
632
633
634
635
636
637
638
639
640
641
642
643
644
645
646
647
648
649
650
651
652
653
654
655
656
657
658
659
660
661
662
663
664
665
666
667
668
669
670
671
672
673
674
675
676
677
678
679
680
681
682
683
684
685
686
687
688
689
690
691
692
693
694
695
696
697
698
699
700
701
702
703
704
705
706
707
708
709
710
711
712
713
714
715
716
717
718
719
720
721
722
723
724
725
726
727
728
729
730
731
732
733
734
735
736
737
738
739
740
741
742
743
744
745
746
747
748
749
750
751
752
753
754
755
756
757
758
759
760
761
762
763
764
765
766
767
768
769
770
771
772
773
774
775
776
777
778
779
780
781
782
783
784
785
786
787
788
789
790
791
792
793
794
795
796
797
798
799
800
801
802
803
804
805
806
807
808
809
810
811
812
813
814
815
816
817
818
819
820
821
822
823
824
825
826
827
828
829
830
831
832
833
834
835
836
837
838
839
840
841
842
843
844
845
846
847
848
849
850
851
852
853
854
855
856
857
858
859
860
861
862
863
864
865
866
867
868
869
870
871
872
873
874
875
876
877
878
879
880
881
882
883
884
885
886
887
888
889
890
891
892
893
894
895
//! Implementation of various bits and pieces of the `panic!` macro and
//! associated runtime pieces.
//!
//! Specifically, this module contains the implementation of:
//!
//! * Panic hooks
//! * Executing a panic up to doing the actual implementation
//! * Shims around "try"
#![deny(unsafe_op_in_unsafe_fn)]
use core::panic::{Location, PanicPayload};
// make sure to use the stderr output configured
// by libtest in the real copy of std
#[cfg(test)]
use realstd::io::try_set_output_capture;
use crate::any::Any;
#[cfg(not(test))]
use crate::io::try_set_output_capture;
use crate::mem::{self, ManuallyDrop};
use crate::panic::{BacktraceStyle, PanicHookInfo};
use crate::sync::atomic::{Atomic, AtomicBool, Ordering};
use crate::sync::nonpoison::RwLock;
use crate::sys::backtrace;
use crate::sys::stdio::panic_output;
use crate::{fmt, intrinsics, process, thread};
// This forces codegen of the function called by panic!() inside the std crate, rather than in
// downstream crates. Primarily this is useful for rustc's codegen tests, which rely on noticing
// complete removal of panic from generated IR. Since begin_panic is inline(never), it's only
// codegen'd once per crate-graph so this pushes that to std rather than our codegen test crates.
//
// (See https://github.com/rust-lang/rust/pull/123244 for more info on why).
//
// If this is causing problems we can also modify those codegen tests to use a crate type like
// cdylib which doesn't export "Rust" symbols to downstream linkage units.
#[unstable(feature = "libstd_sys_internals", reason = "used by the panic! macro", issue = "none")]
#[doc(hidden)]
#[allow(dead_code)]
#[used(compiler)]
pub static EMPTY_PANIC: fn(&'static str) -> ! =
begin_panic::<&'static str> as fn(&'static str) -> !;
// Binary interface to the panic runtime that the standard library depends on.
//
// The standard library is tagged with `#![needs_panic_runtime]` (introduced in
// RFC 1513) to indicate that it requires some other crate tagged with
// `#![panic_runtime]` to exist somewhere. Each panic runtime is intended to
// implement these symbols (with the same signatures) so we can get matched up
// to them.
//
// One day this may look a little less ad-hoc with the compiler helping out to
// hook up these functions, but it is not this day!
#[allow(improper_ctypes)]
unsafe extern "C" {
#[rustc_std_internal_symbol]
fn __rust_panic_cleanup(payload: *mut u8) -> *mut (dyn Any + Send + 'static);
}
unsafe extern "Rust" {
/// `PanicPayload` lazily performs allocation only when needed (this avoids
/// allocations when using the "abort" panic runtime).
#[rustc_std_internal_symbol]
fn __rust_start_panic(payload: &mut dyn PanicPayload) -> u32;
}
/// This function is called by the panic runtime if FFI code catches a Rust
/// panic but doesn't rethrow it. We don't support this case since it messes
/// with our panic count.
#[cfg(not(test))]
#[rustc_std_internal_symbol]
extern "C" fn __rust_drop_panic() -> ! {
rtabort!("Rust panics must be rethrown");
}
/// This function is called by the panic runtime if it catches an exception
/// object which does not correspond to a Rust panic.
#[cfg(not(test))]
#[rustc_std_internal_symbol]
extern "C" fn __rust_foreign_exception() -> ! {
rtabort!("Rust cannot catch foreign exceptions");
}
#[derive(Default)]
enum Hook {
#[default]
Default,
Custom(Box<dyn Fn(&PanicHookInfo<'_>) + 'static + Sync + Send>),
}
impl Hook {
#[inline]
fn into_box(self) -> Box<dyn Fn(&PanicHookInfo<'_>) + 'static + Sync + Send> {
match self {
Hook::Default => Box::new(default_hook),
Hook::Custom(hook) => hook,
}
}
}
static HOOK: RwLock<Hook> = RwLock::new(Hook::Default);
/// Registers a custom panic hook, replacing the previously registered hook.
///
/// The panic hook is invoked when a thread panics, but before the panic runtime
/// is invoked. As such, the hook will run with both the aborting and unwinding
/// runtimes.
///
/// The default hook, which is registered at startup, prints a message to standard error and
/// generates a backtrace if requested. This behavior can be customized using the `set_hook` function.
/// The current hook can be retrieved while reinstating the default hook with the [`take_hook`]
/// function.
///
/// [`take_hook`]: ./fn.take_hook.html
///
/// The hook is provided with a `PanicHookInfo` struct which contains information
/// about the origin of the panic, including the payload passed to `panic!` and
/// the source code location from which the panic originated.
///
/// The panic hook is a global resource.
///
/// # Panics
///
/// Panics if called from a panicking thread.
///
/// # Examples
///
/// The following will print "Custom panic hook":
///
/// ```should_panic
/// use std::panic;
///
/// panic::set_hook(Box::new(|_| {
/// println!("Custom panic hook");
/// }));
///
/// panic!("Normal panic");
/// ```
#[stable(feature = "panic_hooks", since = "1.10.0")]
pub fn set_hook(hook: Box<dyn Fn(&PanicHookInfo<'_>) + 'static + Sync + Send>) {
if thread::panicking() {
panic!("cannot modify the panic hook from a panicking thread");
}
// Drop the old hook after changing the hook to avoid deadlocking if its
// destructor panics.
drop(HOOK.replace(Hook::Custom(hook)));
}
/// Unregisters the current panic hook and returns it, registering the default hook
/// in its place.
///
/// *See also the function [`set_hook`].*
///
/// [`set_hook`]: ./fn.set_hook.html
///
/// If the default hook is registered it will be returned, but remain registered.
///
/// # Panics
///
/// Panics if called from a panicking thread.
///
/// # Examples
///
/// The following will print "Normal panic":
///
/// ```should_panic
/// use std::panic;
///
/// panic::set_hook(Box::new(|_| {
/// println!("Custom panic hook");
/// }));
///
/// let _ = panic::take_hook();
///
/// panic!("Normal panic");
/// ```
#[must_use]
#[stable(feature = "panic_hooks", since = "1.10.0")]
pub fn take_hook() -> Box<dyn Fn(&PanicHookInfo<'_>) + 'static + Sync + Send> {
if thread::panicking() {
panic!("cannot modify the panic hook from a panicking thread");
}
HOOK.replace(Hook::Default).into_box()
}
/// Atomic combination of [`take_hook`] and [`set_hook`]. Use this to replace the panic handler with
/// a new panic handler that does something and then executes the old handler.
///
/// [`take_hook`]: ./fn.take_hook.html
/// [`set_hook`]: ./fn.set_hook.html
///
/// # Panics
///
/// Panics if called from a panicking thread.
///
/// # Examples
///
/// The following will print the custom message, and then the normal output of panic.
///
/// ```should_panic
/// #![feature(panic_update_hook)]
/// use std::panic;
///
/// // Equivalent to
/// // let prev = panic::take_hook();
/// // panic::set_hook(Box::new(move |info| {
/// // println!("...");
/// // prev(info);
/// // }));
/// panic::update_hook(move |prev, info| {
/// println!("Print custom message and execute panic handler as usual");
/// prev(info);
/// });
///
/// panic!("Custom and then normal");
/// ```
#[unstable(feature = "panic_update_hook", issue = "92649")]
pub fn update_hook<F>(hook_fn: F)
where
F: Fn(&(dyn Fn(&PanicHookInfo<'_>) + Send + Sync + 'static), &PanicHookInfo<'_>)
+ Sync
+ Send
+ 'static,
{
if thread::panicking() {
panic!("cannot modify the panic hook from a panicking thread");
}
let mut hook = HOOK.write();
let prev = mem::take(&mut *hook).into_box();
*hook = Hook::Custom(Box::new(move |info| hook_fn(&prev, info)));
}
/// The default panic handler.
#[optimize(size)]
fn default_hook(info: &PanicHookInfo<'_>) {
// If this is a double panic, make sure that we print a backtrace
// for this panic. Otherwise only print it if logging is enabled.
let backtrace = if info.force_no_backtrace() {
None
} else if panic_count::get_count() >= 2 {
BacktraceStyle::full()
} else {
crate::panic::get_backtrace_style()
};
// The current implementation always returns `Some`.
let location = info.location().unwrap();
let msg = payload_as_str(info.payload());
let write = #[optimize(size)]
|err: &mut dyn crate::io::Write| {
// Use a lock to prevent mixed output in multithreading context.
// Some platforms also require it when printing a backtrace, like `SymFromAddr` on Windows.
let mut lock = backtrace::lock();
thread::with_current_name(|name| {
let name = name.unwrap_or("<unnamed>");
let tid = thread::current_os_id();
// Try to write the panic message to a buffer first to prevent other concurrent outputs
// interleaving with it.
let mut buffer = [0u8; 512];
let mut cursor = crate::io::Cursor::new(&mut buffer[..]);
let write_msg = |dst: &mut dyn crate::io::Write| {
// We add a newline to ensure the panic message appears at the start of a line.
writeln!(dst, "\nthread '{name}' ({tid}) panicked at {location}:\n{msg}")
};
if write_msg(&mut cursor).is_ok() {
let pos = cursor.position() as usize;
let _ = err.write_all(&buffer[0..pos]);
} else {
// The message did not fit into the buffer, write it directly instead.
let _ = write_msg(err);
};
});
static FIRST_PANIC: Atomic<bool> = AtomicBool::new(true);
match backtrace {
Some(BacktraceStyle::Short) => {
drop(lock.print(err, crate::backtrace_rs::PrintFmt::Short))
}
Some(BacktraceStyle::Full) => {
drop(lock.print(err, crate::backtrace_rs::PrintFmt::Full))
}
Some(BacktraceStyle::Off) => {
if FIRST_PANIC.swap(false, Ordering::Relaxed) {
let _ = writeln!(
err,
"note: run with `RUST_BACKTRACE=1` environment variable to display a \
backtrace"
);
if cfg!(miri) {
let _ = writeln!(
err,
"note: in Miri, you may have to set `MIRIFLAGS=-Zmiri-env-forward=RUST_BACKTRACE` \
for the environment variable to have an effect"
);
}
}
}
// If backtraces aren't supported or are forced-off, do nothing.
None => {}
}
};
if let Ok(Some(local)) = try_set_output_capture(None) {
write(&mut *local.lock().unwrap_or_else(|e| e.into_inner()));
try_set_output_capture(Some(local)).ok();
} else if let Some(mut out) = panic_output() {
write(&mut out);
}
}
#[cfg(not(test))]
#[doc(hidden)]
#[cfg(panic = "immediate-abort")]
#[unstable(feature = "update_panic_count", issue = "none")]
pub mod panic_count {
/// A reason for forcing an immediate abort on panic.
#[derive(Debug)]
pub enum MustAbort {
AlwaysAbort,
PanicInHook,
}
#[inline]
pub fn increase(run_panic_hook: bool) -> Option<MustAbort> {
None
}
#[inline]
pub fn finished_panic_hook() {}
#[inline]
pub fn decrease() {}
#[inline]
pub fn set_always_abort() {}
// Disregards ALWAYS_ABORT_FLAG
#[inline]
#[must_use]
pub fn get_count() -> usize {
0
}
#[must_use]
#[inline]
pub fn count_is_zero() -> bool {
true
}
}
#[cfg(not(test))]
#[doc(hidden)]
#[cfg(not(panic = "immediate-abort"))]
#[unstable(feature = "update_panic_count", issue = "none")]
pub mod panic_count {
use crate::cell::Cell;
use crate::sync::atomic::{Atomic, AtomicUsize, Ordering};
const ALWAYS_ABORT_FLAG: usize = 1 << (usize::BITS - 1);
/// A reason for forcing an immediate abort on panic.
#[derive(Debug)]
pub enum MustAbort {
AlwaysAbort,
PanicInHook,
}
// Panic count for the current thread and whether a panic hook is currently
// being executed..
thread_local! {
static LOCAL_PANIC_COUNT: Cell<(usize, bool)> = const { Cell::new((0, false)) }
}
// Sum of panic counts from all threads. The purpose of this is to have
// a fast path in `count_is_zero` (which is used by `panicking`). In any particular
// thread, if that thread currently views `GLOBAL_PANIC_COUNT` as being zero,
// then `LOCAL_PANIC_COUNT` in that thread is zero. This invariant holds before
// and after increase and decrease, but not necessarily during their execution.
//
// Additionally, the top bit of GLOBAL_PANIC_COUNT (GLOBAL_ALWAYS_ABORT_FLAG)
// records whether panic::always_abort() has been called. This can only be
// set, never cleared.
// panic::always_abort() is usually called to prevent memory allocations done by
// the panic handling in the child created by `libc::fork`.
// Memory allocations performed in a child created with `libc::fork` are undefined
// behavior in most operating systems.
// Accessing LOCAL_PANIC_COUNT in a child created by `libc::fork` would lead to a memory
// allocation. Only GLOBAL_PANIC_COUNT can be accessed in this situation. This is
// sufficient because a child process will always have exactly one thread only.
// See also #85261 for details.
//
// This could be viewed as a struct containing a single bit and an n-1-bit
// value, but if we wrote it like that it would be more than a single word,
// and even a newtype around usize would be clumsy because we need atomics.
// But we use such a tuple for the return type of increase().
//
// Stealing a bit is fine because it just amounts to assuming that each
// panicking thread consumes at least 2 bytes of address space.
static GLOBAL_PANIC_COUNT: Atomic<usize> = AtomicUsize::new(0);
// Increases the global and local panic count, and returns whether an
// immediate abort is required.
//
// This also updates thread-local state to keep track of whether a panic
// hook is currently executing.
#[must_use = "MustAbort may not be ignored"]
pub fn increase(run_panic_hook: bool) -> Option<MustAbort> {
let global_count = GLOBAL_PANIC_COUNT.fetch_add(1, Ordering::Relaxed);
if global_count & ALWAYS_ABORT_FLAG != 0 {
// Do *not* access thread-local state, we might be after a `fork`.
return Some(MustAbort::AlwaysAbort);
}
LOCAL_PANIC_COUNT.with(|c| {
let (count, in_panic_hook) = c.get();
if in_panic_hook {
return Some(MustAbort::PanicInHook);
}
c.set((count + 1, run_panic_hook));
None
})
}
pub fn finished_panic_hook() {
LOCAL_PANIC_COUNT.with(|c| {
let (count, _) = c.get();
c.set((count, false));
});
}
pub fn decrease() {
GLOBAL_PANIC_COUNT.fetch_sub(1, Ordering::Relaxed);
LOCAL_PANIC_COUNT.with(|c| {
let (count, _) = c.get();
c.set((count - 1, false));
});
}
pub fn set_always_abort() {
GLOBAL_PANIC_COUNT.fetch_or(ALWAYS_ABORT_FLAG, Ordering::Relaxed);
}
// Disregards ALWAYS_ABORT_FLAG
#[must_use]
pub fn get_count() -> usize {
LOCAL_PANIC_COUNT.with(|c| c.get().0)
}
// Disregards ALWAYS_ABORT_FLAG
#[must_use]
#[inline]
pub fn count_is_zero() -> bool {
if GLOBAL_PANIC_COUNT.load(Ordering::Relaxed) & !ALWAYS_ABORT_FLAG == 0 {
// Fast path: if `GLOBAL_PANIC_COUNT` is zero, all threads
// (including the current one) will have `LOCAL_PANIC_COUNT`
// equal to zero, so TLS access can be avoided.
//
// In terms of performance, a relaxed atomic load is similar to a normal
// aligned memory read (e.g., a mov instruction in x86), but with some
// compiler optimization restrictions. On the other hand, a TLS access
// might require calling a non-inlinable function (such as `__tls_get_addr`
// when using the GD TLS model).
true
} else {
is_zero_slow_path()
}
}
// Slow path is in a separate function to reduce the amount of code
// inlined from `count_is_zero`.
#[inline(never)]
#[cold]
fn is_zero_slow_path() -> bool {
LOCAL_PANIC_COUNT.with(|c| c.get().0 == 0)
}
}
#[cfg(test)]
pub use realstd::rt::panic_count;
/// Invoke a closure, capturing the cause of an unwinding panic if one occurs.
#[cfg(panic = "immediate-abort")]
pub unsafe fn catch_unwind<R, F: FnOnce() -> R>(f: F) -> Result<R, Box<dyn Any + Send>> {
Ok(f())
}
/// Invoke a closure, capturing the cause of an unwinding panic if one occurs.
#[cfg(not(panic = "immediate-abort"))]
pub unsafe fn catch_unwind<R, F: FnOnce() -> R>(f: F) -> Result<R, Box<dyn Any + Send>> {
union Data<F, R> {
f: ManuallyDrop<F>,
r: ManuallyDrop<R>,
p: ManuallyDrop<Box<dyn Any + Send>>,
}
// We do some sketchy operations with ownership here for the sake of
// performance. We can only pass pointers down to `do_call` (can't pass
// objects by value), so we do all the ownership tracking here manually
// using a union.
//
// We go through a transition where:
//
// * First, we set the data field `f` to be the argumentless closure that we're going to call.
// * When we make the function call, the `do_call` function below, we take
// ownership of the function pointer. At this point the `data` union is
// entirely uninitialized.
// * If the closure successfully returns, we write the return value into the
// data's return slot (field `r`).
// * If the closure panics (`do_catch` below), we write the panic payload into field `p`.
// * Finally, when we come back out of the `try` intrinsic we're
// in one of two states:
//
// 1. The closure didn't panic, in which case the return value was
// filled in. We move it out of `data.r` and return it.
// 2. The closure panicked, in which case the panic payload was
// filled in. We move it out of `data.p` and return it.
//
// Once we stack all that together we should have the "most efficient'
// method of calling a catch panic whilst juggling ownership.
let mut data = Data { f: ManuallyDrop::new(f) };
// SAFETY:
//
// Access to the union's fields: this is `std` and we know that the `catch_unwind`
// intrinsic fills in the `r` or `p` union field based on its return value.
//
// The call to `intrinsics::catch_unwind` is made safe by:
// - `do_call`, the first argument, can be called with the initial `data_ptr`.
// - `do_catch`, the second argument, can be called with the `data_ptr` as well.
// See their safety preconditions for more information
unsafe {
return if intrinsics::catch_unwind(do_call, &raw mut data, do_catch) {
Err(ManuallyDrop::into_inner(data.p))
} else {
Ok(ManuallyDrop::into_inner(data.r))
};
}
// We consider unwinding to be rare, so mark this function as cold. However,
// do not mark it no-inline -- that decision is best to leave to the
// optimizer (in most cases this function is not inlined even as a normal,
// non-cold function, though, as of the writing of this comment).
#[cold]
#[optimize(size)]
unsafe fn cleanup(payload: *mut u8) -> Box<dyn Any + Send + 'static> {
// SAFETY: The whole unsafe block hinges on a correct implementation of
// the panic handler `__rust_panic_cleanup`. As such we can only
// assume it returns the correct thing for `Box::from_raw` to work
// without undefined behavior.
let obj = unsafe { Box::from_raw(__rust_panic_cleanup(payload)) };
panic_count::decrease();
obj
}
// SAFETY:
// data must be non-NUL, correctly aligned, and a pointer to a `Data<F, R>`
// Its must contains a valid `f` (type: F) value that can be use to fill
// `data.r`.
#[inline]
unsafe fn do_call<F: FnOnce() -> R, R>(data: *mut Data<F, R>) {
// SAFETY: this is the responsibility of the caller, see above.
unsafe {
let f = ManuallyDrop::take(&mut (*data).f);
(*data).r = ManuallyDrop::new(f());
}
}
// We *do* want this part of the catch to be inlined: this allows the
// compiler to properly track accesses to the Data union and optimize it
// away most of the time.
//
// SAFETY:
// data must be non-NUL, correctly aligned, and a pointer to a `Data<F, R>`
// Since this uses `cleanup` it also hinges on a correct implementation of
// `__rustc_panic_cleanup`.
#[inline]
#[rustc_nounwind] // `intrinsic::catch_unwind` requires catch fn to be nounwind
unsafe fn do_catch<F: FnOnce() -> R, R>(data: *mut Data<F, R>, payload: *mut u8) {
// SAFETY: this is the responsibility of the caller, see above.
//
// When `__rustc_panic_cleaner` is correctly implemented we can rely
// on `obj` being the correct thing to pass to `data.p` (after wrapping
// in `ManuallyDrop`).
unsafe {
let obj = cleanup(payload);
(*data).p = ManuallyDrop::new(obj);
}
}
}
/// Determines whether the current thread is unwinding because of panic.
#[inline]
pub fn panicking() -> bool {
!panic_count::count_is_zero()
}
/// Entry point of panics from the core crate (`panic_impl` lang item).
#[cfg(not(any(test, doctest)))]
#[panic_handler]
pub fn panic_handler(info: &core::panic::PanicInfo<'_>) -> ! {
struct FormatStringPayload<'a> {
inner: &'a core::panic::PanicMessage<'a>,
string: Option<String>,
}
impl FormatStringPayload<'_> {
fn fill(&mut self) -> &mut String {
let inner = self.inner;
// Lazily, the first time this gets called, run the actual string formatting.
self.string.get_or_insert_with(|| {
let mut s = String::new();
let mut fmt = fmt::Formatter::new(&mut s, fmt::FormattingOptions::new());
let _err = fmt::Display::fmt(&inner, &mut fmt);
s
})
}
}
unsafe impl PanicPayload for FormatStringPayload<'_> {
fn take_box(&mut self) -> *mut (dyn Any + Send) {
// We do two allocations here, unfortunately. But (a) they're required with the current
// scheme, and (b) we don't handle panic + OOM properly anyway (see comment in
// begin_panic below).
let contents = mem::take(self.fill());
Box::into_raw(Box::new(contents))
}
fn get(&mut self) -> &(dyn Any + Send) {
self.fill()
}
}
impl fmt::Display for FormatStringPayload<'_> {
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
if let Some(s) = &self.string {
f.write_str(s)
} else {
fmt::Display::fmt(&self.inner, f)
}
}
}
struct StaticStrPayload(&'static str);
unsafe impl PanicPayload for StaticStrPayload {
fn take_box(&mut self) -> *mut (dyn Any + Send) {
Box::into_raw(Box::new(self.0))
}
fn get(&mut self) -> &(dyn Any + Send) {
&self.0
}
fn as_str(&mut self) -> Option<&str> {
Some(self.0)
}
}
impl fmt::Display for StaticStrPayload {
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
f.write_str(self.0)
}
}
let loc = info.location().unwrap(); // The current implementation always returns Some
let msg = info.message();
crate::sys::backtrace::__rust_end_short_backtrace(move || {
if let Some(s) = msg.as_str() {
panic_with_hook(
&mut StaticStrPayload(s),
loc,
info.can_unwind(),
info.force_no_backtrace(),
);
} else {
panic_with_hook(
&mut FormatStringPayload { inner: &msg, string: None },
loc,
info.can_unwind(),
info.force_no_backtrace(),
);
}
})
}
/// This is the entry point of panicking for the non-format-string variants of
/// panic!() and assert!(). In particular, this is the only entry point that supports
/// arbitrary payloads, not just format strings.
#[unstable(feature = "libstd_sys_internals", reason = "used by the panic! macro", issue = "none")]
#[cfg_attr(not(any(test, doctest)), lang = "begin_panic")]
// lang item for CTFE panic support
// never inline unless panic=immediate-abort to avoid code
// bloat at the call sites as much as possible
#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold, optimize(size))]
#[cfg_attr(panic = "immediate-abort", inline)]
#[track_caller]
#[rustc_do_not_const_check] // hooked by const-eval
pub const fn begin_panic<M: Any + Send>(msg: M) -> ! {
if cfg!(panic = "immediate-abort") {
intrinsics::abort()
}
struct Payload<A> {
inner: Option<A>,
}
unsafe impl<A: Send + 'static> PanicPayload for Payload<A> {
fn take_box(&mut self) -> *mut (dyn Any + Send) {
// Note that this should be the only allocation performed in this code path. Currently
// this means that panic!() on OOM will invoke this code path, but then again we're not
// really ready for panic on OOM anyway. If we do start doing this, then we should
// propagate this allocation to be performed in the parent of this thread instead of the
// thread that's panicking.
let data = match self.inner.take() {
Some(a) => Box::new(a) as Box<dyn Any + Send>,
None => process::abort(),
};
Box::into_raw(data)
}
fn get(&mut self) -> &(dyn Any + Send) {
match self.inner {
Some(ref a) => a,
None => process::abort(),
}
}
}
impl<A: 'static> fmt::Display for Payload<A> {
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
match &self.inner {
Some(a) => f.write_str(payload_as_str(a)),
None => process::abort(),
}
}
}
let loc = Location::caller();
crate::sys::backtrace::__rust_end_short_backtrace(move || {
panic_with_hook(
&mut Payload { inner: Some(msg) },
loc,
/* can_unwind */ true,
/* force_no_backtrace */ false,
)
})
}
fn payload_as_str(payload: &dyn Any) -> &str {
if let Some(&s) = payload.downcast_ref::<&'static str>() {
s
} else if let Some(s) = payload.downcast_ref::<String>() {
s.as_str()
} else {
"Box<dyn Any>"
}
}
/// Central point for dispatching panics.
///
/// Executes the primary logic for a panic, including checking for recursive
/// panics, panic hooks, and finally dispatching to the panic runtime to either
/// abort or unwind.
#[optimize(size)]
fn panic_with_hook(
payload: &mut dyn PanicPayload,
location: &'static Location<'static>,
can_unwind: bool,
force_no_backtrace: bool,
) -> ! {
let must_abort = panic_count::increase(true);
// Check if we need to abort immediately.
if let Some(must_abort) = must_abort {
match must_abort {
panic_count::MustAbort::PanicInHook => {
// Don't try to format the message in this case, perhaps that is causing the
// recursive panics. However if the message is just a string, no user-defined
// code is involved in printing it, so that is risk-free.
let message: &str = payload.as_str().unwrap_or_default();
rtprintpanic!(
"panicked at {location}:\n{message}\nthread panicked while processing panic. aborting.\n"
);
}
panic_count::MustAbort::AlwaysAbort => {
// Unfortunately, this does not print a backtrace, because creating
// a `Backtrace` will allocate, which we must avoid here.
rtprintpanic!("aborting due to panic at {location}:\n{payload}\n");
}
}
crate::process::abort();
}
match *HOOK.read() {
// Some platforms (like wasm) know that printing to stderr won't ever actually
// print anything, and if that's the case we can skip the default
// hook. Since string formatting happens lazily when calling `payload`
// methods, this means we avoid formatting the string at all!
// (The panic runtime might still call `payload.take_box()` though and trigger
// formatting.)
Hook::Default if panic_output().is_none() => {}
Hook::Default => {
default_hook(&PanicHookInfo::new(
location,
payload.get(),
can_unwind,
force_no_backtrace,
));
}
Hook::Custom(ref hook) => {
hook(&PanicHookInfo::new(location, payload.get(), can_unwind, force_no_backtrace));
}
}
// Indicate that we have finished executing the panic hook. After this point
// it is fine if there is a panic while executing destructors, as long as it
// it contained within a `catch_unwind`.
panic_count::finished_panic_hook();
if !can_unwind {
// If a thread panics while running destructors or tries to unwind
// through a nounwind function (e.g. extern "C") then we cannot continue
// unwinding and have to abort immediately.
rtprintpanic!("thread caused non-unwinding panic. aborting.\n");
crate::process::abort();
}
rust_panic(payload)
}
/// This is the entry point for `resume_unwind`.
/// It just forwards the payload to the panic runtime.
#[cfg_attr(panic = "immediate-abort", inline)]
pub fn resume_unwind(payload: Box<dyn Any + Send>) -> ! {
if let Some(must_abort) = panic_count::increase(false) {
match must_abort {
panic_count::MustAbort::PanicInHook => {
rtprintpanic!("thread panicked while processing panic. aborting.\n");
}
panic_count::MustAbort::AlwaysAbort => {
rtprintpanic!("aborting due to panic\n");
}
}
crate::process::abort();
}
struct RewrapBox(Box<dyn Any + Send>);
unsafe impl PanicPayload for RewrapBox {
fn take_box(&mut self) -> *mut (dyn Any + Send) {
Box::into_raw(mem::replace(&mut self.0, Box::new(())))
}
fn get(&mut self) -> &(dyn Any + Send) {
&*self.0
}
}
impl fmt::Display for RewrapBox {
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
f.write_str(payload_as_str(&self.0))
}
}
rust_panic(&mut RewrapBox(payload))
}
/// A function with a fixed suffix (through `rustc_std_internal_symbol`)
/// on which to slap yer breakpoints.
#[inline(never)]
#[cfg_attr(not(test), rustc_std_internal_symbol)]
#[cfg(not(panic = "immediate-abort"))]
fn rust_panic(msg: &mut dyn PanicPayload) -> ! {
let code = unsafe { __rust_start_panic(msg) };
rtabort!("failed to initiate panic, error {code}")
}
#[cfg_attr(not(test), rustc_std_internal_symbol)]
#[cfg(panic = "immediate-abort")]
fn rust_panic(_: &mut dyn PanicPayload) -> ! {
crate::intrinsics::abort();
}

## Footer

© 2026 GitHub, Inc.

### Footer navigation
* [Terms][103]
* [Privacy][104]
* [Security][105]
* [Status][106]
* [Community][107]
* [Docs][108]
* [Contact][109]
* Manage cookies
* Do not share my personal information

You can’t perform that action at this time.

[1]: #start-of-content
[2]: /login?return_to=https%3A%2F%2Fgithub.com%2Frust-lang%2Frust%2Fblob%2Fmain%2Flibrary%2Fstd%2Fsrc%2Fpanicking.rs
[3]: https://github.com/features/copilot
[4]: https://github.com/features/ai/github-app
[5]: https://github.com/mcp
[6]: https://github.com/features/actions
[7]: https://github.com/features/codespaces
[8]: https://github.com/features/issues
[9]: https://github.com/features/code-review
[10]: https://github.com/security/advanced-security
[11]: https://github.com/security/advanced-security/code-security
[12]: https://github.com/security/advanced-security/secret-protection
[13]: https://github.com/why-github
[14]: https://docs.github.com
[15]: https://github.blog
[16]: https://github.blog/changelog
[17]: https://github.com/marketplace
[18]: https://github.com/features
[19]: https://github.com/enterprise
[20]: https://github.com/team
[21]: https://github.com/enterprise/startups
[22]: https://github.com/solutions/industry/nonprofits
[23]: https://github.com/solutions/use-case/app-modernization
[24]: https://github.com/solutions/use-case/devsecops
[25]: https://github.com/solutions/use-case/devops
[26]: https://github.com/solutions/use-case/ci-cd
[27]: https://github.com/solutions/use-case
[28]: https://github.com/solutions/industry/healthcare
[29]: https://github.com/solutions/industry/financial-services
[30]: https://github.com/solutions/industry/manufacturing
[31]: https://github.com/solutions/industry/government
[32]: https://github.com/solutions/industry
[33]: https://github.com/solutions
[34]: https://github.com/resources/articles?topic=ai
[35]: https://github.com/resources/articles?topic=software-development
[36]: https://github.com/resources/articles?topic=devops
[37]: https://github.com/resources/articles?topic=security
[38]: https://github.com/resources/articles
[39]: https://github.com/customer-stories
[40]: https://github.com/resources/events
[41]: https://github.com/resources/whitepapers
[42]: https://github.com/solutions/executive-insights
[43]: https://skills.github.com
[44]: https://docs.github.com
[45]: https://support.github.com
[46]: https://github.com/orgs/community/discussions
[47]: https://github.com/trust-center
[48]: https://github.com/partners
[49]: https://github.com/resources
[50]: https://github.com/sponsors
[51]: https://securitylab.github.com
[52]: https://maintainers.github.com
[53]: https://github.com/accelerator
[54]: https://stars.github.com
[55]: https://archiveprogram.github.com
[56]: https://github.com/topics
[57]: https://github.com/trending
[58]: https://github.com/collections
[59]: https://github.com/enterprise
[60]: https://github.com/security/advanced-security
[61]: https://github.com/features/copilot/copilot-business
[62]: https://github.com/premium-support
[63]: https://github.com/pricing
[64]: https://docs.github.com/search-github/github-code-search/understanding-github-code-search-syntax
[65]: https://docs.github.com/search-github/github-code-search/understanding-github-code-search-syntax
[66]: /login?return_to=https%3A%2F%2Fgithub.com%2Frust-lang%2Frust%2Fblob%2Fmain%2Flibrary%2Fstd%2Fsrc%2Fpanicking.rs
[67]: /signup?ref_cta=Sign+up&ref_loc=header+logged+out&ref_page=%2F%3Cuser-name%3E%2F%3Crepo-name%3E%2Fblob%2Fshow&sour
ce=header-repo&source_repo=rust-lang%2Frust
[68]: 
[69]: 
[70]: 
[71]: 
[72]: /rust-lang
[73]: /rust-lang/rust
[74]: 
[75]: /login?return_to=%2Frust-lang%2Frust
[76]: /login?return_to=%2Frust-lang%2Frust
[77]: /login?return_to=%2Frust-lang%2Frust
[78]: /rust-lang/rust
[79]: /rust-lang/rust/issues
[80]: /rust-lang/rust/pulls
[81]: /rust-lang/rust/actions
[82]: /rust-lang/rust/projects
[83]: /rust-lang/rust/security
[84]: /rust-lang/rust/pulse
[85]: /rust-lang/rust
[86]: /rust-lang/rust/issues
[87]: /rust-lang/rust/pulls
[88]: /rust-lang/rust/actions
[89]: /rust-lang/rust/projects
[90]: /rust-lang/rust/security
[91]: /rust-lang/rust/pulse
[92]: /rust-lang/rust/tree/main
[93]: /rust-lang/rust/tree/main/library
[94]: /rust-lang/rust/tree/main/library/std
[95]: /rust-lang/rust/tree/main/library/std/src
[96]: /rust-lang/rust/commits/main/library/std/src/panicking.rs
[97]: /rust-lang/rust/commits/main/library/std/src/panicking.rs
[98]: /rust-lang/rust/tree/main
[99]: /rust-lang/rust/tree/main/library
[100]: /rust-lang/rust/tree/main/library/std
[101]: /rust-lang/rust/tree/main/library/std/src
[102]: https://github.com/rust-lang/rust/raw/refs/heads/main/library/std/src/panicking.rs
[103]: https://docs.github.com/site-policy/github-terms/github-terms-of-service
[104]: https://docs.github.com/site-policy/privacy-policies/github-privacy-statement
[105]: https://github.com/security
[106]: https://www.githubstatus.com/
[107]: https://github.community/
[108]: https://docs.github.com/
[109]: https://support.github.com?tags=dotcom-footer
```
