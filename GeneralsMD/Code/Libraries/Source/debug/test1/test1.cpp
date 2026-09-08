/////////////////////////////////////////////////////////////////////////EA-V1
// $File: //depot/GeneralsMD/Staging/code/Libraries/Source/debug/test1/test1.cpp $
// $Author: mhoffe $
// $Revision: #1 $
// $DateTime: 2003/07/03 11:55:26 $
//
// ©2003 Electronic Arts
//
// Debug module - Test 1 (Checking early exceptions)
//////////////////////////////////////////////////////////////////////////////
#include "../debug.h"

const char *DebugGetDefaultCommands(void)
{
  return "!debug.io con add";
}

int divByNull;
unsigned *invalidPtr=(unsigned *)0x666;

bool crash(void)
{
  *invalidPtr/=divByNull;
  return true;
}

bool thisWillCrash=crash();

void main(void)
{
}
