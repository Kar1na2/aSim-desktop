# aSim-desktop
Taking inspiration and creating a desktop browser (later app) version of aSim (https://asim.sh/) 

**This space will be used to verbose my thoughts and motivations and will be refromatted later to be proper README after everything has been made** 

## **Current motivations**
- problem: aSim is a social app that is on mobile only (android / ios), being able to show demos on browser 
- solution: Creating a desktop app that can simulate part of aSim 

This leads to our new segment 
## **Goals / Features / Design (?)** 
- in order to stay true to aSim, the App's format will look like a phone, percisely the dimensions will be similar to iphone mirroring from Mac 
- From a limited perspective on aSim, the 2 main features this app will contain are 
    - Global interaction such as 
        - Graffiti Wall 
        - Community Postboards, questionnaire 
    - Selection of "vibes" into a template for profile boards

## **TimeLine** - This will be showing major commits made in order to build 
### 3/21

## **Verbose** - My thought process when building this app
- To first build this app, I need users need login information basic stuff like username and password to start
    - To get the username and password I need to setup a database
        - The database I'm going to use is dynamoDB from my local machine first and connect to it through Rust
