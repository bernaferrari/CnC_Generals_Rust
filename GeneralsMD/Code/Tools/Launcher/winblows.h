#ifndef WINBLOWS_HEADER
#define WINBLOWS_HEADER

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <windowsx.h>
#include <stdlib.h>

#include"wstypes.h"

  extern HINSTANCE Global_instance;
  extern LPSTR     Global_commandline;
  extern int       Global_commandshow;

  extern int       main(int argc, char *argv[]);

  int              Print_WM(UINT wm,char *out);

#endif
