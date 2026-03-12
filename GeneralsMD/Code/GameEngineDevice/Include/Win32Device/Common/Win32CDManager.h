//
// Project:    Generals
//
// Module:     Game Engine DEvice Win32 Common
//
// File name:  Win32Device/Common/Win32CDManager.h
//
// Created:    11/26/01 TR
//
//----------------------------------------------------------------------------

#pragma once

#ifndef _WIN32DEVICE_COMMON_WIN32CDMANAGER_H_
#define _WIN32DEVICE_COMMON_WIN32CDMANAGER_H_


//----------------------------------------------------------------------------
//           Includes                                                      
//----------------------------------------------------------------------------

#include "Common/CDManager.h"


//----------------------------------------------------------------------------
//           Forward References
//----------------------------------------------------------------------------



//----------------------------------------------------------------------------
//           Type Defines
//----------------------------------------------------------------------------

//===============================
// Win32CDDrive 
//===============================

class Win32CDDrive : public CDDrive
{
	public:

	Win32CDDrive();
	virtual ~Win32CDDrive();

	virtual void refreshInfo( void );					///< Update drive with least 

};

//===============================
// Win32CDManager
//===============================

class Win32CDManager : public CDManager
{
	public:

		Win32CDManager();
		virtual ~Win32CDManager();

		// sub system operations
		virtual void init( void );
		virtual void update( void );
		virtual void reset( void );
		virtual void refreshDrives( void );				///< Refresh drive info

	protected:

		virtual CDDriveInterface* createDrive( void );
};

//----------------------------------------------------------------------------
//           Inlining                                                       
//----------------------------------------------------------------------------


#endif // _WIN32DEVICE_COMMON_WIN32CDMANAGER_H_
