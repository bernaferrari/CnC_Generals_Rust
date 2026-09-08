/////////////////////////////////////////////////////////////////////////EA-V1
// $File: //depot/GeneralsMD/Staging/code/Libraries/Source/debug/internal_except.h $
// $Author: mhoffe $
// $Revision: #1 $
// $DateTime: 2003/07/03 11:55:26 $
//
// ©2003 Electronic Arts
//
// Unhandled exception handler
//////////////////////////////////////////////////////////////////////////////
#ifdef _MSC_VER
#  pragma once
#endif
#ifndef INTERNAL_EXCEPT_H // Include guard
#define INTERNAL_EXCEPT_H

/// \internal exception handler
class DebugExceptionhandler
{
  DebugExceptionhandler(const DebugExceptionhandler&);
  DebugExceptionhandler& operator=(const DebugExceptionhandler&);

  // nobody can instantiate us
  DebugExceptionhandler(void);

  /** \internal

    \brief Log exception location.

    \param dbg debug instance
    \param exptr exception pointers
  */
  static void LogExceptionLocation(Debug &dbg, struct _EXCEPTION_POINTERS *exptr);

  /** \internal

    \brief Log regular registers.

    \param dbg debug instance
    \param exptr exception pointers
  */
  static void LogRegisters(Debug &dbg, struct _EXCEPTION_POINTERS *exptr);

  /** \internal

    \brief Log FPU registers.

    \param dbg debug instance
    \param exptr exception pointers
  */
  static void LogFPURegisters(Debug &dbg, struct _EXCEPTION_POINTERS *exptr);

public:
  
  /** \internal

    \brief Determine exception type.

    \param exptr exception pointers
    \param explanation exception explanation, buffer must be 512 chars
    \return exception type as string
  */
  static const char *GetExceptionType(struct _EXCEPTION_POINTERS *exptr, char *explanation);

  /** \internal 
  
    \brief System exception filter
  */
  static long __stdcall ExceptionFilter(struct _EXCEPTION_POINTERS* pExPtrs);
};

#endif // INTERNAL_EXCEPT_H
