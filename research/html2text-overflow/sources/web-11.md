# Web source

- URL: https://www.reddit.com/r/cpp_questions/comments/10khigh/why_would_size_t_subtraction_cause_overflow
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-29T16:21:15.415201507+00:00

```text
[ Skip to main content ][1]
Open menu Open navigation [ ][2]Go to Reddit Home
[ Sign Up ][3]Sign up for Reddit [ Log In ][4]Log in to Reddit
Expand user menu Open settings menu
[
Go to cpp_questions ][5]
[r/cpp_questions][6]
• 3y ago
[jiboxiake][7]

# Why would size_t subtraction cause overflow?

[
OPEN
][8]

I have a code snippet doing binary search like this

`Highway_Vertices_Pair fake_pair(source,destination,-1);`
`size_t low = 0;`
`size_t high = highwayPairs.size()-1;`
`while(low<=high){`
`size_t mid = (high+low)/2;`
`if(highwayPairs.at(mid)==fake_pair){`
`return std::pair<float,size_t>(highwayPairs.at(mid).distance,mid);`
`}else if(highwayPairs.at(mid)<fake_pair){`
`low = mid +1;`
`}else{`
`high = mid-1;`
`}`
`}`

where `highwayPairs` is a vector that I want to do binary search on. Also the vector will not be modified after initial
setup. During query phase I got an error like:

`terminate called after throwing an instance of 'std::out_of_range'`

`what(): vector:: _M_range_check: __n (which is 9223372036854775807) >= this->size() (which is 132238)`

And I found out it was `mid` that has overflow and it is caused by the `high`. Any idea what caused that? Thanks.

Share

# People also ask about section

People also ask about
Common pitfalls for C++ beginners

Jump into C++ development by **focusing on learning resources like learncpp.com and applying your knowledge to
projects** to avoid common pitfalls.

### Choose Quality Learning Resources
* **Prioritize learncpp.com**: Many Redditors consistently recommend learncpp.com as the best free tutorial for C++,
  covering basics to advanced topics with modern best practices. "www.learncpp.com is the best free tutorial out there."
* **Supplement with books**: While online tutorials are valuable, established books from experts like Bjarne Stroustrup
  can offer deeper insights. "If you want to learn sth. properly get a good book on the topic from experts like B.
  Stroustrup, N. Josuttis or S. Myers, M. Gregoire."
* **Be wary of certain resources**: Avoid cplusplus.com, W3Schools, GeeksforGeeks, and most YouTube tutorials, as they
  often contain outdated or poor quality information. "Most youtube/video tutorials are of low quality, I would
  recommend to stay away from them as well."

### Focus on Foundational Concepts and Modern C++
* **Master the basics**: Understand core C++ concepts thoroughly, including memory management and object-oriented
  programming. "Learn Cpp is not for mastering the language it goes into basics and into the basics of those things."
* **Embrace modern C++**: Familiarize yourself with features from C++11 and later standards, such as move semantics,
  smart pointers, ranges, and lambdas, as these are crucial for writing efficient and maintainable code. "For C++11, it
  was `unique_ptr` (it needed move semantics)"
* **Understand C++'s complexity**: C++ is a vast language with many nuances; recognize that continuous learning is part
  of the journey. "Bjarne doesn't know all of C++ and he created the language. C++ is like the Mariana Trench."

### Apply Learning Through Projects
* **Build practical applications**: Actively work on projects to solidify your understanding and gain practical
  experience. "It can’t hurt as an introduction. Your best way to learn though is to actually write code."
* **Break down complex problems**: Divide large projects into smaller, manageable tasks and iterate on them. "You sit
  down and break the big problem (the whole application) into smaller and more manageable little problems and
  iteratively keep building out your project."
* **Ask questions and seek existing solutions**: Don't hesitate to ask for help or research if a solution already
  exists, especially for common problems. "If you encounter what seems like a simple or common problem, don't be afraid
  to ask if there already is an existing solution to it."

Are you ready to commit to consistent practice and learning to overcome these C++ challenges?

[ Show More ][9]
[
Best practices for memory management in C++
][10]
[
How to optimize C++ code for performance
][11]
[
Understanding C++ templates and their uses
][12]
[
Differences between C++11 and C++14 features
][13]
Public

Anyone can view, post, and comment to this community

0 0

## Top Posts
* [
  Reddit
  reReddit: Top posts of January 24, 2023
  ][14]
* [
  Reddit
  reReddit: Top posts of January 2023
  ][15]
* [
  Reddit
  reReddit: Top posts of 2023
  ][16]
* [Home][17]
* [Popular][18]
* [News][19]
* [Explore][20]
* [Best of Reddit][21]
* [Best of Reddit in Portuguese][22]
* [Best of Reddit in German][23]
* [Reddit Rules][24]
* [Privacy Policy][25]
* [User Agreement][26]
* [Accessibility][27]
* [Reddit, Inc. © 2026. All rights reserved.][28]

Join the most real place on the internet

Continue with Phone Number
Continue with Email

By continuing, you agree to our [User Agreement][29] and acknowledge that you understand the [Privacy Policy][30].

[1]: #main-content
[2]: https://www.reddit.com/
[3]: https://www.reddit.com/register/
[4]: https://www.reddit.com/login/
[5]: https://www.reddit.com/r/cpp_questions/
[6]: https://www.reddit.com/r/cpp_questions/
[7]: https://www.reddit.com/user/jiboxiake/
[8]: /r/cpp_questions/?f=flair_name%3A%22OPEN%22
[9]: https://www.reddit.com/answers/7089ad51-e2c7-48a0-bc5b-817496cd58ca/?q=Common+pitfalls+for+C%2B%2B+beginners&source
=PDP
[10]: https://www.reddit.com/answers/e6bd6f9c-f46f-4fc0-8fb1-94b40fa16ec4/?q=Best+practices+for+memory+management+in+C%2
B%2B&source=PDP
[11]: https://www.reddit.com/answers/9b572577-d576-4906-862d-eaaaaaf626fc/?q=How+to+optimize+C%2B%2B+code+for+performanc
e&source=PDP
[12]: https://www.reddit.com/answers/fdb56365-00ad-49e4-8d2d-6fb2891088db/?q=Understanding+C%2B%2B+templates+and+their+u
ses&source=PDP
[13]: https://www.reddit.com/answers/c869dba6-1822-4dd8-af2d-946235e6fd8b/?q=Differences+between+C%2B%2B11+and+C%2B%2B14
+features&source=PDP
[14]: https://www.reddit.com/posts/2023/january-24-1/global/
[15]: https://www.reddit.com/posts/2023/january/global/
[16]: https://www.reddit.com/posts/2023/global/
[17]: /?feed=home
[18]: /r/popular/
[19]: /news/
[20]: /explore/
[21]: https://www.reddit.com/posts/2026/global/
[22]: https://www.reddit.com/posts/2026/tl-pt-BR/
[23]: https://www.reddit.com/posts/2026/tl-de/
[24]: https://www.redditinc.com/policies/content-policy
[25]: https://www.reddit.com/policies/privacy-policy
[26]: https://www.redditinc.com/policies/user-agreement
[27]: https://support.reddithelp.com/hc/sections/38303584022676-Accessibility
[28]: https://redditinc.com
[29]: https://www.redditinc.com/policies/user-agreement
[30]: https://www.redditinc.com/policies/privacy-policy
```
