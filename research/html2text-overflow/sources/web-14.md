# Web source

- URL: https://discuss.python.org/t/how-may-we-avoid-overflow-errors/14606
- Title: [Discussions on Python.org][1]
- Captured (UTC): 2026-06-29T16:21:19.896934151+00:00

```text
[Discussions on Python.org][1]

# [How may we avoid overflow errors?][2]

[ Python Help ][3]
[john316][4] (John M) March 26, 2022, 2:42am 1

Hi,
I am trying to find Power(x, n) when n is large by recursive binary division on n. I tried two methods, the first of
which has an overflow error but the second works well. Why? What’s the difference?

First:

`class Solution50:
    def myPow(self, x: float, n: int) -> float:
        if n == 0:
            return 1
        if n == 1:
            return x
        if n < 0 :
            return 1/self.myPow(x, -n)
        if n % 2 == 0:
            return (self.myPow(x**2, n//2)) 
        if n % 2 == 1:
            return x * (self.myPow(x**2, n//2)) 

sol50 = Solution50()
sol50.myPow(2.00, -2147483648)
`

Second:

`class Solution50f:
    def myPow(self, x: float, n: int) -> float:
        if n == 0:
            return 1.0
        elif n < 0:
            return 1 / self.myPow(x, -n)
        elif n % 2:
            return self.myPow(x * x, n // 2) * x
        else:
            return self.myPow(x * x, n // 2)

sol50f = Solution50f()
sol50f.myPow(2.00, -2147483648)
`

[steven.daprano][5] (Steven D'Aprano) March 26, 2022, 3:51am 2

Hi John,

The two methods behave differently because they are written differently, one uses the exponentiation operator `**` and
the other just uses multiplication.

Your first method tries to compute the sequence of powers:
* `2.0**2` → 4.0
* `4.0**2` → 16.0
* `16.0**2` → 256.0
* `256.0**2` → 65536.0
* `65536.0**2` → 4294967296.0
* `4294967296.0**2` → 1.8446744073709552e+19
* `1.8446744073709552e+19**2` → 3.402823669209385e+38
* `3.402823669209385e+38**2` → 1.157920892373162e+77
* `1.157920892373162e+77**2` → 1.3407807929942597e+154
* `1.3407807929942597e+154**2` → OverflowError

While the second computes the similar sequence, but using multiplication. For brevity I won’t write them all down in
full:
* `2.0*2.0` → 4.0
* `4.0*4.0` → 16.0
* `16.0*16.0` → 256.0
* etc.
* `1.3407807929942597e+154*1.3407807929942597e+154` → float(‘inf’)

and from that point on, each time you multiply `inf*inf` you just get another infinity. Then finally you get:

`1/inf
`

which rounds to zero, so you get the final result of 0.0.

Why the difference between multiplication and exponentiation?

*shrug*

That is probably buried deep, deep in the mists of history, when Python was first written. Floating point multiplication
in Python must follow the rules set up by the IEEE-754 standard, which has multiplication overflow to the special value
float(‘inf’).

But for reasons now lost, whoever designed Python’s exponent operator, and the `pow()` function, made a different
decision to raise OverflowError instead.

There’s probably no real logic behind the difference. Its probably just a case of somebody thought it was a good idea at
the time.

By the way, each of the builtin exponent operator `**`, the `pow()` builtin function, and `math.pow` function round 2.0
to the power of (negative huge number) to 0.0.

[john316][6] (John M) March 26, 2022, 5:32am 3

Thank you Steven! After I modified the first by following your comments, it work as well as the second:

`class Solution:
    def myPow(self, x: float, n: int) -> float:
        if n == 0:
            return 1
        if n == 1:
            return x
        if n < 0 :
            return 1/self.myPow(x, -n)
        if n % 2 == 0:
            return (self.myPow(x*x, n//2)) 
        if n % 2 == 1:
            return x * (self.myPow(x*x, n//2)) 

`

[steven.daprano][7] (Steven D'Aprano) March 26, 2022, 6:02am 4

By the way, you have two classes which do nothing and exist for no reason other than to hold a function. This is a poor
design choice. It reads like you are trying to program Java in Python.

In Java, every function has to be put into a method of some class. The classes often don’t do anything except hold that
method. This blog post might help:

[http://steve-yegge.blogspot.com/2006/03/execution-in-kingdom-of-nouns.html][8]

The general guideline is that every class should have both *state* (data) and *behaviour* (one or more methods). If a
class has no state (like your Solution50 class), or no methods, then chances are very good that the class isn’t pulling
its weight and should be removed.

(Classes which only provide state with no, or minimal, behaviour are probably more common. In another language, we might
use a record or a struct for these method-less values.)

So instead of making myPow a method of some class, you should consider just making it a function. If you need more than
one myPow, you can give them a suffix or prefix:

`def mypow_using_exponentiation(x, n):
    ...

def mypow_using_multiplication(x, n):
    ...
`

which helps describe what they do.

[ljh][9] (ljh) March 26, 2022, 7:46am 5

Hi Steven,

Does float have underflow too? What is the smallest float and how can I test against it?

int and float are both numbers, why float has limits but int does not?

int and Decimal have all the good, what are their disadvantages?

Thanks

`from decimal import Decimal

f: float =            1.3407807929942597e+154  **2  # overflow error
i: int = int(         1.3407807929942597e+154) **2
d: Decimal = Decimal('1.3407807929942597e+154')**2
`

[ljh][10] (ljh) March 26, 2022, 8:18am 6

Hi Steven,

What does the 34 mean in the error message?

Thanks

`f = 1.3407807929942597e154**2
f = 1.9907807929942597e154**2

# OverflowError: (34, 'Result too large')
`

[steven.daprano][11] (Steven D'Aprano) March 26, 2022, 10:10pm 7

“What does the 34 mean in the error message?”

I don’t know.

[https://bugs.python.org/issue47134][12]

[steven.daprano][13] (Steven D'Aprano) March 27, 2022, 12:39am 8

The 34 is the C errno code returned by the platform’s maths library.

When Python calls the C pow() function from the maths library, it can set an error code. If errno is set, the only
*expected* error code is ERANGE, numeric result out of range, but due to bugs in the various C maths libraries, it could
be anything.

So if the error code is ERANGE, OverflowError is raised, otherwise ValueError is raised. In either case, both the
numeric code and standard C error message are shown.

[steven.daprano][14] (Steven D'Aprano) March 27, 2022, 6:21am 9

Good question!

Yes, floats underflow too.

Using IEEE-754 floats, the smallest possible normal float is 2.2250738585072014e-308; the smallest denormal is 5e-324.

`sys.float_info.min  # 2.2250738585072014e-308 is the smallest norm.

math.nextafter(0.0, 1.0)  # 5e-324 is the smallest non-zero denorm.
`

So once you get below 2.225e-308 you have what is called “gradual overflow”, you get denormal numbers. And once you get
to 5e-324 the computation will underflow to zero.

`5e-324 / 2  # returns 0.0
`

[people.eecs.berkeley.edu][15]

### [ARITH_17U.pdf][16]

119.83 KB

In my opinion, the best argument for gradual underflow (and denormals) is that with it, `x == y` implies `x - y == 0.0`
and vice versa.

Without gradual underflow, there are numbers where `x != y` but `x-y == 0`.

[ljh][17] (ljh) March 27, 2022, 6:55am 10

Thank you so much, Steven!

`from decimal import Decimal

f: float =            1.3407807929942597e+154  **2  # overflow error
i: int = int(         1.3407807929942597e+154) **2
d: Decimal = Decimal('1.3407807929942597e+154')**2
`

float has limits while int and Decimal do not.
Is it because int and Decimal use string to represent numbers inside and will this make operation on int and Decimal a
bit slow.

[cameron][18] (Cameron Simpson) March 27, 2022, 10:00am 11

By ljh via Discussions on [Python.org][19] at 26Mar2022 08:28:

> What does the 34 mean in the error message?
> 
> # OverflowError: (34, ‘Result too large’)

OverfloewError will be a subclass of OSError, and on UNIX systems error
34 is usually this:

`#define    ERANGE        34    /* Math result not representable */
`

So you will find that that exception has a .errno attribute equal to 34.

Cheers,
Cameron Simpson [cs@cskk.id.au][20]

[steven.daprano][21] (Steven D'Aprano) March 27, 2022, 10:38am 12

If I remember correctly, int uses an array of bytes as a base-256 number. So it is limited only by the amount of memory
available.

Decimal is a floating point number like float, the big differences are that it uses base-10 and it has user-configurable
arbitrary precision. There are still hard limits: the maximum precision is 425000000 digits on 32-bit platforms, and
999999999999999999 digits on 64-bit platforms.

Although I imagine you might run out of memory long before you can work with that many digits. For example, trying to
print a string of 999999999999999999 digits would require more than 888 petabytes of memory just for the string.

The built-in float type is an object-oriented wrapper on the platform C double type, a 64 bit floating point number. It
is fixed to the precision of C doubles.

But both Decimal and float have the same overflow and underflow issues. They just occur at different values.

`Decimal(0).next_plus()    # smallest denorm Decimal('1E-1000026') with default settings
Decimal(0).next_plus()/2  # underflows to zero
`

Decimal instances also use more than four times the memory of a float:

`sys.getsizeof(Decimal(0))/sys.getsizeof(0.0)
# returns 4.333333333333333
`

Also many float operations are performed directly by the CPU, and are very fast, while Decimal has to be emulated in
software.

`import timeit
t1 = timeit.Timer('x/2', setup='x = 1.0')
t2 = timeit.Timer('x/2', setup='from decimal import Decimal as D; x = D(1)')

print( min(t1.repeat()) )
print( min(t2.repeat()) )
`

On my PC, that prints approximately 0.028 for the float, and 0.154 for the Decimal. So float is about 5 or 6 times
faster.

[ljh][22] (ljh) March 27, 2022, 1:01pm 13

Thanks, Steven!

[eryksun][23] (Eryk Sun) March 27, 2022, 1:17pm 14
cameron:

> OverfloewError will be a subclass of OSError

`OverflowError` is a subclass of `ArithmeticError`. It has no `errno` attribute, but the exception in this case is
created with a C `errno` value and related message. For example:

`>>> raise OverflowError(errno.ERANGE, os.strerror(errno.ERANGE))
Traceback (most recent call last):
  File "<stdin>", line 1, in <module>
OverflowError: (34, 'Numerical result out of range')
`

Without already being familiar with the `errno` value, the message, or the fact that libm `pow()` was called, there’s
really no suggestion to check `errno.errorcode[34]` to discover that 34 is `ERANGE`.

[eryksun][24] (Eryk Sun) March 27, 2022, 1:25pm 15
steven.daprano:

> If I remember correctly, int uses an array of bytes as a base-256 number. So it is limited only by the amount of
> memory available.

The `int` type is almost always built with 30-bit digits (C macro `PYLONG_BITS_IN_DIGIT`), though it also supports
15-bit digits. Here’s an excerpt from “Include/cpython/longintrepr.h”:

`/* Parameters of the integer representation.  There are two different
   sets of parameters: one set for 30-bit digits, stored in an unsigned 32-bit
   integer type, and one set for 15-bit digits with each digit stored in an
   unsigned short.  The value of PYLONG_BITS_IN_DIGIT, defined either at
   configure time or in pyport.h, is used to decide which digit size to use.

   Type 'digit' should be able to hold 2*PyLong_BASE-1, and type 'twodigits'
   should be an unsigned integer type able to hold all integers up to
   PyLong_BASE*PyLong_BASE-1.  x_sub assumes that 'digit' is an unsigned type,
   and that overflow is handled by taking the result modulo 2**N for some N >
   PyLong_SHIFT.  The majority of the code doesn't care about the precise
   value of PyLong_SHIFT, but there are some notable exceptions:

   - PyLong_{As,From}ByteArray require that PyLong_SHIFT be at least 8

   - long_hash() requires that PyLong_SHIFT is *strictly* less than the number
     of bits in an unsigned long, as do the PyLong <-> long (or unsigned long)
     conversion functions

   - the Python int <-> size_t/Py_ssize_t conversion functions expect that
     PyLong_SHIFT is strictly less than the number of bits in a size_t

   - the marshal code currently expects that PyLong_SHIFT is a multiple of 15

   - NSMALLNEGINTS and NSMALLPOSINTS should be small enough to fit in a single
     digit; with the current values this forces PyLong_SHIFT >= 9

  The values 15 and 30 should fit all of the above requirements, on any
  platform.
*/
`

[steven.daprano][25] (Steven D'Aprano) March 28, 2022, 9:37am 16

Ah, thanks Eryk Sun.

I wonder where I got the idea that CPython used base-256?
* [Home ][26]
* [Categories ][27]
* [Guidelines ][28]
* [Terms of Service ][29]
* [Privacy Policy ][30]

Powered by [Discourse][31], best viewed with JavaScript enabled

[1]: /
[2]: /t/how-may-we-avoid-overflow-errors/14606
[3]: /c/help/7
[4]: https://discuss.python.org/u/john316
[5]: https://discuss.python.org/u/steven.daprano
[6]: https://discuss.python.org/u/john316
[7]: https://discuss.python.org/u/steven.daprano
[8]: http://steve-yegge.blogspot.com/2006/03/execution-in-kingdom-of-nouns.html
[9]: https://discuss.python.org/u/ljh
[10]: https://discuss.python.org/u/ljh
[11]: https://discuss.python.org/u/steven.daprano
[12]: https://bugs.python.org/issue47134
[13]: https://discuss.python.org/u/steven.daprano
[14]: https://discuss.python.org/u/steven.daprano
[15]: https://people.eecs.berkeley.edu/~wkahan/ARITH_17U.pdf
[16]: https://people.eecs.berkeley.edu/~wkahan/ARITH_17U.pdf
[17]: https://discuss.python.org/u/ljh
[18]: https://discuss.python.org/u/cameron
[19]: http://Python.org
[20]: mailto:cs@cskk.id.au
[21]: https://discuss.python.org/u/steven.daprano
[22]: https://discuss.python.org/u/ljh
[23]: https://discuss.python.org/u/eryksun
[24]: https://discuss.python.org/u/eryksun
[25]: https://discuss.python.org/u/steven.daprano
[26]: /
[27]: /categories
[28]: /guidelines
[29]: /tos
[30]: /privacy
[31]: https://www.discourse.org
```
