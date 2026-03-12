#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef WWHACK_H
#define WWHACK_H

/*
** FORCE_LINK is a hack to force a module in a lib to be linked into the EXE.
*/				
#define		FORCE_LINK(	module )				void _Force_Link_ ## module( void );	_Force_Link_ ## module()
#define		DECLARE_FORCE_LINK(	module )	void _Force_Link_ ## module( void ) {}

#endif
