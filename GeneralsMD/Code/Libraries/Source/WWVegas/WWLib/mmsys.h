#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef MMSYS_H
#define MMSYS_H

/*
** This header just includes mmsystem.h with warning 4201 disabled
*/

#pragma warning(disable:4201)
#include <mmsystem.h>
#pragma warning(default:4201)

#endif // MMSYS_H
