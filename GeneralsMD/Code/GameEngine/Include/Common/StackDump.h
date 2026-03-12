#pragma once

#ifndef __STACKDUMP_H_
#define __STACKDUMP_H_

#ifndef IG_DEGBUG_STACKTRACE
#define IG_DEBUG_STACKTRACE	1
#endif // Unsure about this one -ML 3/25/03
#if defined(_DEBUG) || defined(_INTERNAL) || defined(IG_DEBUG_STACKTRACE)

// Writes a stackdump (provide a callback : gets called per line)
// If callback is NULL then will write using OuputDebugString
void StackDump(void (*callback)(const char*));

// Writes a stackdump (provide a callback : gets called per line)
// If callback is NULL then will write using OuputDebugString
void StackDumpFromContext(DWORD eip,DWORD esp,DWORD ebp, void (*callback)(const char*));

// Gets count* addresses from the current stack
void FillStackAddresses(void**addresses, unsigned int count, unsigned int skip = 0);

// Do full stack dump using an address array
void StackDumpFromAddresses(void**addresses, unsigned int count, void (*callback)(const char*));

void GetFunctionDetails(void *pointer, char*name, char*filename, unsigned int* linenumber, unsigned int* address);

// Dumps out the exception info and stack trace.
void DumpExceptionInfo( unsigned int u, EXCEPTION_POINTERS* e_info );

#else

__inline void StackDump(void (*callback)(const char*)) {};

// Gets count* addresses from the current stack
__inline void FillStackAddresses(void**addresses, unsigned int count, unsigned int skip = 0) {}

// Do full stack dump using an address array
__inline void StackDumpFromAddresses(void**addresses, unsigned int count, void (*callback)(const char*)) {}

__inline void GetFunctionDetails(void *pointer, char*name, char*filename, unsigned int* linenumber, unsigned int* address) {}

// Dumps out the exception info and stack trace.
__inline void DumpExceptionInfo( unsigned int u, EXCEPTION_POINTERS* e_info ) {};

#endif

extern AsciiString g_LastErrorDump;
#endif // __STACKDUMP_H_
