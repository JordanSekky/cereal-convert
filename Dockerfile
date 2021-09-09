FROM ekidd/rust-musl-builder:stable as builder

ARG APP_NAME=cereal

WORKDIR /home/rust/
RUN cargo new --bin ${APP_NAME} 
WORKDIR /home/rust/${APP_NAME}
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release
RUN rm src/*.rs

ADD . ./

RUN rm ./target/x86_64-unknown-linux-musl/release/deps/${APP_NAME}*
RUN cargo build --release


FROM frolvlad/alpine-glibc:latest

ARG APP=/usr/src/app
ARG APP_NAME=cereal

ENV TZ=Etc/UTC \
    APP_USER=appuser

RUN addgroup -S $APP_USER \
    && adduser -S -g $APP_USER $APP_USER

RUN apk update && \
    apk add --no-cache --upgrade \
    ca-certificates \
    libstdc++ \
    && rm -rf /var/cache/apk/*

RUN mkdir /opt/calibre && wget -q https://download.calibre-ebook.com/5.26.0/calibre-5.26.0-x86_64.txz && tar Jxf calibre-5.26.0-x86_64.txz -C /opt/calibre && /opt/calibre/calibre_postinstall


COPY --from=builder /home/rust/${APP_NAME}/target/x86_64-unknown-linux-musl/release/${APP_NAME} ${APP}/${APP_NAME}

RUN chown -R $APP_USER:$APP_USER ${APP}

USER $APP_USER
WORKDIR ${APP}

CMD ["./cereal"]
