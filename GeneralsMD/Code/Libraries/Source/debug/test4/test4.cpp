/////////////////////////////////////////////////////////////////////////EA-V1
// $File: //depot/GeneralsMD/Staging/code/Libraries/Source/debug/test4/test4.cpp $
// $Author: mhoffe $
// $Revision: #1 $
// $DateTime: 2003/07/03 11:55:26 $
//
// ©2003 Electronic Arts
//
// Debug module - Test 4 (Multiple DASSERTs, high-count DCHECKs)
//////////////////////////////////////////////////////////////////////////////
#include "../debug.h"

void main(void)
{
  for (int i=0;i<30;i++)
    DCHECK_MSG(i>100,"run#" << i);
  Debug::Command("list c");
  for (int k=0;k<3;k++)
  {
    DASSERT(k>4);
    DASSERT_MSG(k<1,"k must be less than 1...");
    Debug::Command("list a");
  }
}
