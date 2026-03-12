#ifndef LOADBMP_HEADER
#define LOADBMP_HEADER

#include<stdlib.h>
#include<stdio.h>
#include "wstypes.h"
#include "winblows.h"


//
// Functions and data assocated with a loaded bitmap on a single window.
//
class LoadBmp
{
 public:
                  LoadBmp();
                 ~LoadBmp();
   bit8           init(char *filename,HWND hwnd);  // must call before the drawBmp
   bit8           drawBmp(void);  // call this from your WM_PAINT message

 private:
   HBITMAP        BitmapHandle_;
   HPALETTE       PalHandle_;
   HWND           WindowHandle_;
};


#endif