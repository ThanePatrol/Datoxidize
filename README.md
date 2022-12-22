# Datoxidize
A self-hosted file syncing solution for your files. 
Mostly incomplete at this stage. Only file syncing is working

## Features
- Syncs your files and folders to your local server
- Locally host your files on your hardware using Docker
- Graphical application to monitor and manage your files
- CLI application also available for macOS and Linux
- No abstractions, your data is stored in a standard format accessible via any device
- 

## Future Features
These features are yet to be added. If you can see yourself adding one, create a pull request!
- iOS app and Android app to act as a front-end for accessing files and syncing
- Optionally turn on a webDav option to allow legacy app access


# todo
#### 1. implement web server view
- serve basic static html pages with a list of the top level directory

#### 2. package app into standalone docker container
- needs to be used and tested as a standalone container
- create container, ensure it works to sync files from local to container

#### 3. create gui clients
- Start out with desktop clients
- iOs/android to follow