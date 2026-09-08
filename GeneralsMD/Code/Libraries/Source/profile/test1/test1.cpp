/////////////////////////////////////////////////////////////////////////EA-V1
// $File: //depot/GeneralsMD/Staging/code/Libraries/Source/profile/test1/test1.cpp $
// $Author: mhoffe $
// $Revision: #3 $
// $DateTime: 2003/07/09 10:57:23 $
//
// ©2003 Electronic Arts
//
// Profile module - Test 1 (basic testing)
//////////////////////////////////////////////////////////////////////////////
#include "../profile.h"
#include "../../debug/debug.h"
#include <stdio.h>

const char *DebugGetDefaultCommands(void)
{
  return "!debug.io con add\ndebug.add l + *\nprofile.result";
}

extern int q;

void calcThis(void)
{
  q++;
}

void calcThat(void)
{
  calcThis();
  q--;
}

// it must be done this "complicated" because
// otherwise VC does not generate a real recursive
// function call for this simple function...
void recursion2(int level);

void recursion(int level)
{
  q+=level;
  if (level<5000)
    recursion2(level+1);
}

void recursion2(int level)
{
  recursion(level);
}

void recursionShell(void)
{
  ProfileHighLevel::Block b("Test block");
  recursion(0);
}

void showResults(void)
{
  ProfileHighLevel::Id id;
  for (unsigned index=0;ProfileHighLevel::EnumProfile(index,id);index++)
    printf("%-16s%-6s %s\n",id.GetName(),id.GetTotalValue(),id.GetUnit());
}

void main(void)
{
  for (int k=0;k<100;k++)
    if (k%2&&k>80)
      calcThat();
    else
      calcThis();

  recursionShell();
  
  showResults();
}

int q;
