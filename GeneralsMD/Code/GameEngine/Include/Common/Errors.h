// FILE: Errors.h 
//
// Project:    RTS3
//
// File name:  Errors.h
//
// Created:    Steven Johnson, August 2001
//
// Desc:       Error codes
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __ERRORS_H_
#define __ERRORS_H_

/**
	An ErrorCode is the repository for failure modes. In almost all situations,
	these values will  be THROWN, not returned as error codes. Feel free
	to add to this list as necessary; however, there should generally be very
	few codes needed.
*/
enum ErrorCode
{
	ERROR_BASE									= 0xdead0001,								// a nice, distinctive value

	ERROR_BUG										= (ERROR_BASE + 0x0000),		///< should not be possible under normal operation
	ERROR_OUT_OF_MEMORY					= (ERROR_BASE + 0x0001),		///< unable to allocate memory.
	ERROR_BAD_ARG								= (ERROR_BASE + 0x0002),		///< generic "bad argument".
	ERROR_INVALID_FILE_VERSION	= (ERROR_BASE + 0x0003),		///< Unrecognized file version.
	ERROR_CORRUPT_FILE_FORMAT		= (ERROR_BASE + 0x0004),		///< Invalid file format.
	ERROR_BAD_INI								= (ERROR_BASE + 0x0005),		///< Bad INI data.
	ERROR_INVALID_D3D						= (ERROR_BASE + 0x0006),    ///< Error initing D3D 

	ERROR_LAST
};

#endif // __ERRORS_H_
