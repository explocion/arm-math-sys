lib:
	@mkdir -p $@

lib/ARM.CMSIS.%.pack: lib
	wget -O $@ "https://github.com/ARM-software/CMSIS_5/releases/download/$*/ARM.CMSIS.$*.pack"

lib/ARM.CMSIS.%: lib/ARM.CMSIS.%.pack
	unzip -f $< -d $@

lib/musl-%.tar.gz: lib
	wget -O $@ "https://musl.libc.org/releases/musl-$*.tar.gz"

lib/musl-%: lib/musl-%.tar.gz
	@mkdir -p $@
	tar -xzf $< -C $@ --strip-components 1
	cd $@ && ./configure CC=arm-none-eabi-gcc arm
	cd $@ && make obj/include/bits/alltypes.h
	cd $@ && make obj/include/bits/syscall.h

.PHONY: clean
clean:
	rm -rf lib/ARM.CMSIS* lib/musl*
