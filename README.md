# 3334-DataFetcher

This project is from the 3334- Systems Programming class, coded in the Rust language.

The program reads a text file with a list of URLs and checks their http status codes, as well as keeping track of the response time and a timestamp.

To run the program, first need to cd to the status checker folder, to do so you can run the following command:

cd status-checker/

Afterwards you can run the program with the following command:

cargo run -- --file urls.txt --workers N --timeout T --retries X

In the command, N is the number of workers the program uses, T is the time in seconds it takes to timeout, and X is the number of retries the program can make if an attempt to check status fails.
Also, urls.txt is the name of the file used within this github repository, but it can be changed to any other filename and also changed directly for urls.

The program handles bad responses to urls and notifies the user of the status if the site is able to be contacted or not.

Along with printing the program results, it creates a .json file with the information collected, as well as the status code, response time, and timestamp.

#Output Example
Here is an image as an example of the output of the program:
![image](https://github.com/user-attachments/assets/7a9bb56c-ce4e-4957-8cd1-8f01c32f4278)

And here another example using 50 urls, using the urls.txt file from the repository:
![image](https://github.com/user-attachments/assets/9cacb924-6c02-4161-bdd6-c825a1a177d6)

In both cases the command used was:
cargo run -- --file urls.txt --workers 4 --timeout 5 --retries 1 
