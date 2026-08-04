/* puff.h
  Copyright (C) 2002-2013 Mark Adler, all rights reserved
  version 2.3, 21 Jan 2013
  This software is provided 'as-is', without any express or implied
  warranty.  In no event will the author be held liable for any damages
 */
#ifndef NIL
#  define NIL ((unsigned char *)0)      
#endif
int puff(unsigned long dictlen,         
         unsigned char *dest,           
         unsigned long *destlen,        
         const unsigned char *source,   
         unsigned long *sourcelen);     
