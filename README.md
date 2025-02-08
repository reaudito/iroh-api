

# Iroh api 

```
cargo run
```


curl -X POST \
  http://localhost:3000/upload \
  -H "Content-Type: multipart/form-data" \
  -F "file=@/path/to/your/file.txt"
  
  /home/amiya/Documents/workspace/shivarthu/working_directory/iroh-api
  
  curl -X POST \
  http://localhost:3000/upload \
  -H "Content-Type: multipart/form-data" \
  -F "file=@/home/amiya/Documents/workspace/shivarthu/working_directory/iroh-api/file.txt"
