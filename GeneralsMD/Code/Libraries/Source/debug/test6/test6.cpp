/////////////////////////////////////////////////////////////////////////EA-V1
// $File: //depot/GeneralsMD/Staging/code/Libraries/Source/debug/test6/test6.cpp $
// $Author: mhoffe $
// $Revision: #1 $
// $DateTime: 2003/07/03 11:55:26 $
//
// ©2003 Electronic Arts
//
// Debug module - Test 6 (SEH, FPO test)
//////////////////////////////////////////////////////////////////////////////
#include "../debug.h"
#include <stdio.h>

int test,divByZero;

void func1(void)
{
  test/=divByZero;
}

void func2(void)
{
  func1();
}

void func3(void)
{
  func2();
}

void main(void)
{
  try
  {
    func3();
  }
  catch (...) 
  {
    printf("This catch clause should not be executed.\n");
  }
}
