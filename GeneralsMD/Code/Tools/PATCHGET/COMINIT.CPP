//
// If you link with this it will automatically call the COM initialization stuff 
//
#include "cominit.h"
#include <stdlib.h>
#include <stdio.h>
#include <windows.h>
#include <objbase.h>

ComInit::ComInit()
{
  HRESULT hRes = CoInitialize(NULL);
  if (SUCCEEDED(hRes)==FALSE)
  {
    MessageBox(NULL,"Can't initialize COM?!?!","Error:",MB_OK);
    exit(0);
  }
}


ComInit::~ComInit()
{
  CoUninitialize();
}

// Creating this instance will setup all COM stuff & do cleanup on program exit
ComInit  Global_COM_Initializer;

