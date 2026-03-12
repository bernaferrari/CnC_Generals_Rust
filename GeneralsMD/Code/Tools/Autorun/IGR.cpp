//
// IGR.cpp - A class used to access the IGR registry settings.
//
// JeffB 7/5/00
//

#include <assert.h>
#include <windows.h>
#include <winreg.h>
//#include "always.h"
#include "IGR.h"


IGROptionsClass *OnlineOptions = NULL;


/*********************************************************************************************
 * IGROptions::Init -- Class initializer. Reads from the registry										*
 *                                                                                           *
 * INPUT:   None																										*
 *                                                                                           *
 * OUTPUT:  bool; Did we read everything OK?																	*
 *                                                                                           *
 * WARNINGS:   none                                                                          *
 *                                                                                           *
 * HISTORY:                                                                                  *
 *   07/05/00 JeffB: Initial coding																				*
 *===========================================================================================*/
bool IGROptionsClass::Init( void )
{
	int	size;
	int	returnValue;
	HKEY	handle;
	char	key[128];
	unsigned long type;

	valid = false;

	// Load the options from the registry
	size = sizeof( int );

	// Setup the key
	strcpy( key, WOLAPI_REG_KEY_BOTTOM );

	// Get a handle to the WOLAPI entry
	if ( RegOpenKeyEx( HKEY_LOCAL_MACHINE, key, 0, KEY_ALL_ACCESS, &handle ) == ERROR_SUCCESS ) {

		// If successful, get the options
		IGROptionsType ReadOptions = 0;

		returnValue = RegQueryValueEx(handle, WOLAPI_REG_KEY_OPTIONS, NULL, 
			 (unsigned long *) &type, (unsigned char *) &ReadOptions, (unsigned long *)&size);

		if (returnValue == ERROR_SUCCESS) {

			// If successful, and we got a DWORD, store options and set the valid flage
			if (type == REG_DWORD) {
				options = ReadOptions;
				valid = true;
			}	
		}

		// Clean up
		RegCloseKey( handle );
	}
	return ( valid );
}

/***********************************************************************************************
 * IGROptions::Is_Auto_Login_Allowed -- Set the passed options in the registry			   *
 *                                                                                             *
 * INPUT:   None													                           *
 *                                                                                             *
 * OUTPUT:  bool; Is the option set															   *
 *                                                                                             *
 * WARNINGS:   none                                                                            *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   07/05/00 JeffB: Initial coding					                                           *
 *=============================================================================================*/
bool IGROptionsClass::Is_Auto_Login_Allowed( void )
{
	return(( options & IGR_NO_AUTO_LOGIN ) == 0 );
}

/***********************************************************************************************
 * IGROptions::Is_Storing_Nicks_Allowed -- Set the passed options in the registry			   *
 *                                                                                             *
 * INPUT:   None													                           *
 *                                                                                             *
 * OUTPUT:  bool; Is the option set															   *
 *                                                                                             *
 * WARNINGS:   none                                                                            *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   07/05/00 JeffB: Initial coding					                                           *
 *=============================================================================================*/
bool IGROptionsClass::Is_Storing_Nicks_Allowed( void )
{
	return(( options & IGR_NEVER_STORE_NICKS ) == 0 );
}

/***********************************************************************************************
 * IGROptions::Is_Running_Reg_App_Allowed -- Set the passed options in the registry			   *
 *                                                                                             *
 * INPUT:   None													                           *
 *                                                                                             *
 * OUTPUT:  bool; Is the option set															   *
 *                                                                                             *
 * WARNINGS:   none                                                                            *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   07/05/00 JeffB: Initial coding					                                           *
 *=============================================================================================*/
bool IGROptionsClass::Is_Running_Reg_App_Allowed( void )
{
	return(( options & IGR_NEVER_RUN_REG_APP ) == 0 );
}

/*********************************************************************************************
 * IGROptions::Set_Options -- Set the passed options in the registry									*
 *																															*
 * INPUT:		Options to set																						*
 *																															*
 * OUTPUT:		bool; Did we set the options successfully													*
 *                                                                                           *
 * WARNINGS:   none                                                                          *
 *                                                                                           *
 * HISTORY:                                                                                  *
 *   07/05/00 JeffB: Initial coding																				*
 *===========================================================================================*/
bool IGROptionsClass::Set_Options( IGROptionsType options )
{
	bool	ReturnValue = false;
	HKEY	handle;
	int		disp;
	char	key[ 128 ];
 
	// We don't care if it's valid, we'll MAKE it valid.
	strcpy( key, WOLAPI_REG_KEY_BOTTOM );

	// Do they have the WOLAPI key?
	if( RegOpenKeyEx( HKEY_LOCAL_MACHINE, key, 0, KEY_ALL_ACCESS, &handle ) != ERROR_SUCCESS ) {

		// If not, make the WOLAPI key
		if( RegCreateKeyEx( HKEY_LOCAL_MACHINE, key, 0, NULL, REG_OPTION_NON_VOLATILE, KEY_ALL_ACCESS, 
			NULL, &handle, (unsigned long *)&disp ) != ERROR_SUCCESS )
			return false;
	}

	if( RegSetValueEx( handle, WOLAPI_REG_KEY_OPTIONS, 0, REG_DWORD, (unsigned char *)&options, sizeof(int)) 
		== ERROR_SUCCESS ) {
		ReturnValue = true;
	}
	RegCloseKey( handle );

	// Reinit the class to make sure we have these settings for later queries.
	Init();

	assert( valid == TRUE );
 
	return ReturnValue;
}
