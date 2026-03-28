# aSim-desktop
Taking inspiration and creating a desktop browser (later app) version of aSim (https://asim.sh/) 

**This space will be used to verbose my thoughts and motivations and will be refromatted later to be proper README after everything has been made** 

## **Current motivations**
- problem: aSim is a social app that is on mobile only (android / ios), being able to show demos on browser 
- solution: Creating a desktop app that can simulate part of aSim 

This leads to our new segment 
## **Goals / Features / Design (?)** 
- in order to stay true to aSim, the App's format will look like a phone, percisely the dimensions will be similar to iphone mirroring from Mac 
- From a limited perspective on aSim, the 3 main features this app will contain are 
    - Global interaction such as 
        - Graffiti Wall 
        - Community Postboards, questionnaire 
    - Selection of "vibes" into a template for profile boards
    - Personal profile portfolio
        - Name 
        - Username
        - Optional information 
            - Star sign 
            - Gender 
            - other emoji Signs to represent themselves 
        - Interests 

## **TimeLine** - This will be showing major commits made in order to build 
### **3/22**
[e42a7d3] 
- connecting database between backend and local dynamoDB will be changed later to support cloud database

[4c730a7] 
- Creating User athentication and registration

### **3/23**


## **Verbose** - My thought process when building this app
- To first build this app, I need users need login information basic stuff like username and password to start
    - To get the username and password I need to setup a database ✔
        - The database I'm going to use is dynamoDB from my local machine first and connect to it through Rust ✔

- The process that registering users will do is 
    - Username password -> *after successfully registering* 
    - "What is your name?" 
    - "What is your dob?" 
    - "What are your interest?" 

in the above process this will basically be initial registration with the UUID being returned and using the UUID to field the rest of the profile into proper database tables   

- The database will be containing 2 separate datbases 
    - UserId and password for authentication and registration 
    - UserId and associated personal information 

- 


<br>
- The user information inspiration will be taken from [TravelHelper App](https://github.com/USF-CS601-Fall25/final-project-Kar1na2) 
    - userID 
    - username
    - password 
    - usersalt



