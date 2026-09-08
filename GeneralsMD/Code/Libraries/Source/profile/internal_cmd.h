/////////////////////////////////////////////////////////////////////////EA-V1
// $File: //depot/GeneralsMD/Staging/code/Libraries/Source/profile/internal_cmd.h $
// $Author: mhoffe $
// $Revision: #1 $
// $DateTime: 2003/07/09 10:57:23 $
//
// ©2003 Electronic Arts
//
// Profile module command interface
//////////////////////////////////////////////////////////////////////////////
#ifdef _MSC_VER
#  pragma once
#endif
#ifndef INTERNAL_CMD_H // Include guard
#define INTERNAL_CMD_H

class ProfileCmdInterface: public DebugCmdInterface
{
  struct Factory
  {
    ProfileResultInterface* (*func)(int, const char * const *);
    const char *name,*arg;
  };

  static unsigned numResIf;
  static Factory *resIf;

  unsigned numResFunc; // optimizer bug: must be declared volatile!
  ProfileResultInterface **resFunc;

public:
  ProfileCmdInterface(void): numResFunc(0), resFunc(0) {}
  
  static void AddResultFunction(ProfileResultInterface* (*func)(int, const char * const *),
                                const char *name, const char *arg);
  void RunResultFunctions(void);

  virtual bool Execute(class Debug& dbg, const char *cmd, CommandMode cmdmode,
                       unsigned argn, const char * const * argv);
  virtual void Delete(void) {}
};

#endif // INTERNAL_CMD_H
