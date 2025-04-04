# Data Availability System 

This repository presents and PoC of p2p system for sharing encrypted blobs of data. 

## Objective

The main purpose of the project is learning and summarizing new knowledge that I've gained during the bootcamp

## High level overview

This projects tries to fulfill two main user stories:

1. As a user I want to share encrypted blob of data through decentralized p2p network. 
2. As a user I want to get my blob of data from any node inside the network

## Structure 

The project contain two libraries: `crypto` and `da-node`.

### `crypto` Library 

This crate represents a minimal version of the [ElGamal encryption](https://en.wikipedia.org/wiki/ElGamal_encryption).

This crate is used by the `da-node` binary to encrypt blob of data when it is uploaded to the node. 

### `da-node` Binary

Used the GraphQl for interaction with the user and gRPC for p2p communication.

User through GraphQl can:
1. Register/Login (the JWT is used for further authorized communication)
2. Add/Delete blob (those actions require authorization) 
3. Get blobs of data 

Via the gRPC the node can: 
1. Announce blob of data 
2. Fetch the blob of data by id 
3. Conduct a handshake with other nodes 
4. Send/Accept delete requests
5. Share node sync status

## Current Project State

Currently, the system is capable of the most basic happy flow which is adding a blob and syncing within hardcoded/handshaked peers.

### Test coverage

- `crypto` crate was thoughtfully tested 
- `da-node` binary was tested manually 

## Usage 

Currently, the system is in alpha stage and due to time scramble a dev-ready usage of the node cannot be setup at the moment.