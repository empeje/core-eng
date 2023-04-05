# P-02 Commit reveal peg operations

## Open questions (WIP)
Do we need Taproot to encode this? In the original proposal, the format is just using P2SH.

Interpreted answer: We need a taproot output because we send the funds to the stackers. (I.e. they need to use FROST).

What prevents the stackers from just claiming the funds for themselves?

Possible solutions:
- Stacks nodes monitor these payloads, and enforce recovery mode in these situations.

## Background

## Description from mini sBTC
The user generates a taproot script that encodes this condition:  “Here’s an 80-byte payload, but ignore it.  Next, if this transaction is mined in the last 144 Bitcoin blocks, then only the peg wallet signers can spend it.  Otherwise, only I can spend it.”
The user broadcasts a transaction with a single P2TR output that corresponds to the script generated in step 1.
The user broadcasts the transaction ID and script to the network of Stackers (i.e. to their `sbtc-signer` binaries).
The Stackers, upon receipt of the transaction ID and script, spend the user’s P2TR by sending it to the peg wallet address.  Otherwise, if the 144 block timeout passes, the user can reclaim the BTC.

## Links

- [SIP-021](https://github.com/stacksgov/sips/blob/56b73eada5ef1b72376f4a230949297b3edcc562/sips/sip-021/sip-021-trustless-two-way-peg-for-bitcoin.md)
- [mini sBTC](https://docs.google.com/document/d/1R33gZupJg0KsY-vRZYbVFwTHRmq2BCIvyPIVeY0JyGM/)
- [OP_DROP proposal](https://docs.google.com/document/d/1EnYEk6gA2w6VfRpT8CcK8mghZRMUEjn2OhHwzdK_9x0)
- [OP_DROP implementation example](https://github.com/FriendsFerdinand/op_drop_example)
