#if defined(_MSC_VER)
#pragma once
#endif

#ifndef W3DERR_H
#define W3DERR_H

#include "always.h"

/*
** These are the error codes which should be returned by all WW3D functions that can fail
*/
enum WW3DErrorType
{
	WW3D_ERROR_OK				= 0,
	WW3D_ERROR_GENERIC,
	WW3D_ERROR_LOAD_FAILED,
	WW3D_ERROR_SAVE_FAILED,
	WW3D_ERROR_WINDOW_NOT_OPEN,
	WW3D_ERROR_INITIALIZATION_FAILED,
};

#endif
