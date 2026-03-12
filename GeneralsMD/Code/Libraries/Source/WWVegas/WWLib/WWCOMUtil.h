/******************************************************************************
*
* NAME
*     $Archive: /Commando/Code/wwlib/WWCOMUtil.h $
*
* DESCRIPTION
*     COM utility functions and macros
*
* PROGRAMMER
*     Denzil E. Long, Jr.
*     $Author: Denzil_l $
*
* VERSION INFO
*     $Revision: 3 $
*     $Modtime: 8/02/01 3:00p $
*
******************************************************************************/

#ifndef __WWCOMUTIL_H__
#define __WWCOMUTIL_H__

#include <oaidl.h>

//! Invoke PropertyGet on IDispatch interface.
HRESULT STDMETHODCALLTYPE Dispatch_GetProperty(IDispatch* object,
		const OLECHAR* propName, VARIANT* result);

//! Invoke PropertyPut on IDispatch interface.
HRESULT STDMETHODCALLTYPE Dispatch_PutProperty(IDispatch* object,
		const OLECHAR* propName, VARIANT* propValue);

//! Invoke Method on IDispatch interface.
HRESULT STDMETHODCALLTYPE Dispatch_InvokeMethod(IDispatch* object,
		const OLECHAR* methodName, DISPPARAMS* params, VARIANT* result);

//! Register COM in-process DLL server
bool RegisterCOMServer(const char* dllName);

//! Unregister COM in-process DLL server
bool UnregisterCOMServer(const char* dllName);

#endif // __WWCOMUTIL_H__
