/****************************************************************************
*
* FILE
*     $Archive:  $
*
* DESCRIPTION
*     Access privilege definitions.
*
* PROGRAMMER
*     Denzil E. Long, Jr.
*     $Author:  $
*
* VERSION INFO
*     $Modtime:  $
*     $Revision:  $
*
****************************************************************************/

#ifndef RIGHTS_H
#define RIGHTS_H

// Access rights
typedef enum
	{
	Rights_ReadOnly = 0,
	Rights_WriteOnly,
	Rights_ReadWrite,
	} ERights;

#endif // RIGHTS_H
