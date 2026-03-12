#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

int Base64_Encode(void const * source, int slen, void * dest, int dlen);
int Base64_Decode(void const * source, int slen, void * dest, int dlen);
