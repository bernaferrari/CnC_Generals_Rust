#ifndef WSTRING_HEADER
#define WSTRING_HEADER

#include <stdio.h>
#include <stdlib.h>
#include "wstypes.h"

class Wstring
{
 public: 
           Wstring();
           Wstring(IN const Wstring &other);
           Wstring(IN char *string);
          ~Wstring();

   void    clear(void);

   bit8    cat(IN char *string);
   bit8    cat(uint32 size,IN char *string);
   bit8    cat(IN Wstring &string);

   void    cellCopy(OUT char *dest, uint32 len);
   char    remove(sint32 pos, sint32 count);
   bit8    removeChar(char c);
   void    removeSpaces(void);
   char   *get(void);
   char    get(uint32 index);
   uint32  length(void);
   bit8    insert(char c, uint32 pos);
   bit8    insert(char *instring, uint32 pos);
   bit8    replace(IN char *replaceThis,IN char *withThis);
   char    set(IN char *str);
   char    set(uint32 size,IN char *str);
   bit8    set(char c, uint32 index);
   void    setSize(sint32 bytes);  // create an empty string
   void    toLower(void);
   void    toUpper(void);
   bit8    truncate(uint32 len);
   bit8    truncate(char c);  // trunc after char c
   sint32  getToken(int offset,char *delim,Wstring &out);
   sint32  getLine(int offset, Wstring &out);

   bit8    operator==(IN char *other);
   bit8    operator==(IN Wstring &other);
   bit8    operator!=(IN char *other);
   bit8    operator!=(IN Wstring &other);

   Wstring  &operator=(IN char *other);
   Wstring  &operator=(IN Wstring &other);
   Wstring  &operator+=(IN char *other);
   Wstring  &operator+=(IN Wstring &other);
   Wstring   operator+(IN char *other);
   Wstring   operator+(IN Wstring &other);

 private:
   char    *str;    // Pointer to allocated string.
};

#endif
