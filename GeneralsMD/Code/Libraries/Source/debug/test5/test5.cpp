/////////////////////////////////////////////////////////////////////////EA-V1
// $File: //depot/GeneralsMD/Staging/code/Libraries/Source/debug/test5/test5.cpp $
// $Author: mhoffe $
// $Revision: #1 $
// $DateTime: 2003/07/03 11:55:26 $
//
// ©2003 Electronic Arts
//
// Debug module - Test 5 (printf style formatting)
//////////////////////////////////////////////////////////////////////////////
#include "../debug.h"

const char *DebugGetDefaultCommands(void)
{
  return "!debug.io con add";
}

void main(void)
{
  // turn on all logs
  Debug::Command("add l + *");

  for (int k=0;k<16;k++)
    DLOG("Testing: " << Debug::Format("0x%04x (%c)",k,'A'+k) << "\n");  
}
