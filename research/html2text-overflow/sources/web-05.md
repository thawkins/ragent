# Web source

- URL: https://www.reddit.com/r/rust/comments/1ciank7/rust_throwing_overflow_for_negating_anything
- Title: [ Skip to main content ][1]
- Captured (UTC): 2026-06-29T16:21:06.835113843+00:00

```text
[ Skip to main content ][1]
Open menu Open navigation [ ][2]Go to Reddit Home
[ Sign Up ][3]Sign up for Reddit [ Log In ][4]Log in to Reddit
Expand user menu Open settings menu
[
Go to rust ][5]
[r/rust][6]
• 2y ago
[NishantD2D][7]

# Rust throwing "overflow" for negating anything beyond 8 (i32)

[
🙋 seeking help & advice
][8]

on the if statement when calling binary_search function and giving argument -num, rust is throwing "attempt to subtract
with overflow" for anything above 7. i32's range is well beyond what I'm using it for so idk what's going on.

pub fn binary_search(nums : &Vec<i32>, k : i32) -> bool{
    let mut start: usize = 0;
    let mut end = nums.len()-1;
    let mut mid = (start+end)/2;
    while start<=end{
        if nums[mid]==k{
            return true;
        }
        else if nums[mid]>k{
            end = mid-1;
        }
        else if nums[mid]<k{
            start = mid+1;
        }
        mid = (start+end)/2;
    }
    false
} 

pub fn find_max_k(mut nums: Vec<i32>) -> i32 {
    nums.sort();
    for i in (0..nums.len()).rev(){
        let num: i32 = nums[i];
        if num>0 && binary_search(&nums, -num){
            return nums[i] as i32;
        }
    }
    -1
}

pub fn main(){
    let nums = vec![-1,10,6,7,-7,1];
    println!("{}", find_max_k(nums));
}

Share

# People also ask about section

People also ask about
[
Understanding integer overflow in Rust
][9]
[
Rust checked_add usage and examples
][10]
[
Rust usize abs_diff stabilized version
][11]
[
Top Rust libraries for web development
][12]
[
Best practices for Rust error handling
][13]
Public

Anyone can view, post, and comment to this community

0 0

## Top Posts
* [
  Reddit
  reReddit: Top posts of May 2, 2024
  ][14]
* [
  Reddit
  reReddit: Top posts of May 2024
  ][15]
* [
  Reddit
  reReddit: Top posts of 2024
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
[5]: https://www.reddit.com/r/rust/
[6]: https://www.reddit.com/r/rust/
[7]: https://www.reddit.com/user/NishantD2D/
[8]: /r/rust/?f=flair_name%3A%22%F0%9F%99%8B%20seeking%20help%20%26%20advice%22
[9]: https://www.reddit.com/answers/492d45c9-efbc-4a1a-81f6-6091db80625e/?q=Understanding+integer+overflow+in+Rust&sourc
e=PDP
[10]: https://www.reddit.com/answers/4a585d2d-54b7-4a71-b3fd-19c48da2cb92/?q=Rust+checked_add+usage+and+examples&source=
PDP
[11]: https://www.reddit.com/answers/fb369ed2-028f-420b-aeff-452ac8f4ef9d/?q=Rust+usize+abs_diff+stabilized+version&sour
ce=PDP
[12]: https://www.reddit.com/answers/ed0e7aeb-f927-44fc-bfe4-c03444be4728/?q=Top+Rust+libraries+for+web+development&sour
ce=PDP
[13]: https://www.reddit.com/answers/db94946f-0399-4106-aa44-99a6de0b4295/?q=Best+practices+for+Rust+error+handling&sour
ce=PDP
[14]: https://www.reddit.com/posts/2024/may-2-1/global/
[15]: https://www.reddit.com/posts/2024/may/global/
[16]: https://www.reddit.com/posts/2024/global/
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
