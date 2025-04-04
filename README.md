# Data Availability System

This repository presents a PoC of a p2p system for sharing encrypted blobs of data.

## Objective

The main purpose of the project is to learn and summarize new knowledge that I gained during the bootcamp.

## High-Level Overview

This project attempts to fulfill two main user stories:

1. As a user, I want to share an encrypted blob of data through a decentralized p2p network.
2. As a user, I want to get my blob of data from any node inside the network.

## Structure

The project contains two libraries: `crypto` and `da-node`.

### `crypto` Library

This crate represents a minimal version of the [ElGamal encryption](https://en.wikipedia.org/wiki/ElGamal_encryption).

This crate is used by the `da-node` binary to encrypt a blob of data when it is uploaded to the node.

### `da-node` Binary

This uses GraphQL for interaction with the user and gRPC for p2p communication.

Through GraphQL, a user can:

1. Register/Login (JWT is used for further authorized communication)
2. Add/Delete a blob (these actions require authorization)
3. Get blobs of data

Via gRPC, the node can:

1. Announce a blob of data
2. Fetch a blob of data by ID
3. Conduct a handshake with other nodes
4. Send/Accept delete requests
5. Share its node sync status

## Current Project State

Currently, the system is capable of the most basic happy path: adding a blob and syncing with hardcoded/handshaked
peers.

### Test Coverage

- The `crypto` crate was thoroughly tested.
- The `da-node` binary was tested manually.

## Usage

Currently, the system is in the alpha stage. Due to time constraints, a developer-ready setup for the node cannot be
provided at the moment.