/////////////////////////////////////////////////////////////////////////EA-V1
// $File: //depot/GeneralsMD/Staging/code/Libraries/Source/profile/profile_result.h $
// $Author: mhoffe $
// $Revision: #1 $
// $DateTime: 2003/07/09 10:57:23 $
//
// ©2003 Electronic Arts
//
// Result function interface and result functions
//////////////////////////////////////////////////////////////////////////////
#ifdef _MSC_VER
#  pragma once
#endif
#ifndef PROFILE_RESULT_H // Include guard
#define PROFILE_RESULT_H

/**
  \brief Result function class.

  Factories for instances of this class are registered using 
  \ref Profile::AddResultFunction. 
*/
class ProfileResultInterface
{
  // no copying
  ProfileResultInterface(const ProfileResultInterface&);
  ProfileResultInterface& operator=(const ProfileResultInterface&);

public:
  /**
    \brief Write out results.

    This function is called on program exit.
  */
  virtual void WriteResults(void)=0;

  /**
    \brief Destroys the current result function.

    Use this function instead of just delete'ing the instance.
  */
  virtual void Delete(void)=0;

protected:
  ProfileResultInterface(void) {}
};

#endif // PROFILE_RESULT_H
