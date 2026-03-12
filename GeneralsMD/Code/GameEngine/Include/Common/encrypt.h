// FILE: Encrypt.h //////////////////////////////////////////////////////
// Ancient Westwood Online password encryption (obfuscation?) code
// Author: Anonymous

#pragma once

#ifndef ENCRYPT_HEADER
#define ENCRYPT_HEADER

// This routine is non-reentrant, as it returns a static buffer!!!
// Valid input is 4-8 characters, and can contain letters and numbers and '.' and '/'

#define MAX_ENCRYPTED_STRING 8

const char *EncryptString(const char *);

#endif


