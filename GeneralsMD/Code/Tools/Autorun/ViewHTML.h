/******************************************************************************
*
* FILE
*     $Archive: /Renegade Setup/Autorun/ViewHTML.h $
*
* DESCRIPTION
*
* PROGRAMMER
*     $Author: Maria_l $
*
* VERSION INFO
*     $Modtime: 8/14/00 7:53p $
*     $Revision: 3 $
*
******************************************************************************/

#ifndef VIEWHTML_H
#define VIEWHTML_H

#include "CallbackHook.h"

bool ViewHTML(const char* url, bool wait = false, CallbackHook& callback = CallbackHook());

#endif // VIEWHTML_H
